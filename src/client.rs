use crate::game::*;
use derivative::Derivative;
use futures::{
    compat::*,
    prelude::*,
    stream::{SplitSink, StreamExt},
};
use log::*;
use serde::Deserialize;
use thespian::Actor;
use warp::{filters::ws, ws::WebSocket};

type ClientSync = SplitSink<Compat01As03Sink<WebSocket, ws::Message>, ws::Message>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Client {
    game: GameControllerProxy,
    id: PlayerId,

    #[derivative(Debug = "ignore")]
    sink: ClientSync,
}

impl Client {
    pub fn start(id: PlayerId, socket: WebSocket, game: GameControllerProxy) -> ClientProxy {
        let (sink, stream) = socket.sink_compat().split();

        // Spawn the actor and a second task to pump incoming messages from the socket.
        let proxy = Client { game, id, sink }.spawn();
        runtime::spawn(pump_messages(proxy.clone(), stream));

        proxy
    }
}

#[thespian::actor]
impl Client {
    pub async fn socket_message(&mut self, message: ClientMessage) {
        debug!("Received message from player {:?}: {:?}", self.id, message);
        self.game.client_message(self.id, message).await;
    }

    pub async fn handle_update(&mut self, update: String) {
        unimplemented!();
    }

    // TODO: Bake this into the client initialization process such that it can't be
    // called outside of the client connection flow.
    //
    // TODO: Don't represent the world state as a string.
    pub async fn send_world_state(&mut self, world: String) {
        self.sink
            .send(ws::Message::text(world))
            .await
            .expect("Failed to send world state to client");
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    HealSelf,

    AttackPlayer { target: PlayerId },
}

async fn pump_messages<S: Stream<Item = Result<ws::Message, warp::Error>> + StreamExt + Unpin>(
    client: ClientProxy,
    stream: S,
) {
    while let Some(message) = stream.next().await {
        let message = message.expect("Error receiving socket message");
        let message = match message.to_str() {
            Ok(message) => message,
            Err(_) => continue,
        };

        trace!("Received message from socket: {:?}", message);
        let message = match serde_json::from_str::<ClientMessage>(&message) {
            Ok(message) => message,
            Err(err) => {
                debug!("Failed to deserialize client message: {}", err);
                continue;
            }
        };

        client.socket_message(message).await;
    }
}
