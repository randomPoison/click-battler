//! Logic for managing client connections.

use crate::game::*;
use actix::prelude::*;
use actix_web_actors::ws;
use log::*;
use std::time::{Duration, Instant};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ClientSocket {
    /// The time the last heartbeat was received from the client.
    heartbeat: Instant,

    /// The address of the game controller.
    game_controller: Addr<GameController>,
}

impl ClientSocket {
    pub fn new(game_controller: Addr<GameController>) -> Self {
        ClientSocket {
            heartbeat: Instant::now(),
            game_controller,
        }
    }

    /// Checks the heartbeat timeout, and sends pings to the client.
    fn check_heartbeat(&mut self, ctx: &mut <Self as Actor>::Context) {
        if Instant::now().duration_since(self.heartbeat) > CLIENT_TIMEOUT {
            info!("Websocket Client heartbeat failed, disconnecting!");
            ctx.stop();
        } else {
            ctx.ping("");
        }
    }
}

impl Actor for ClientSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, ClientSocket::check_heartbeat);
        let future = self.game_controller
            .send(ClientConnected)
            .into_actor(self)
            .then(|player_id, _, _| {
                info!("Connected client given ID {:?}", player_id);
                fut::ok(())
            });
        Arbiter::spawn(future);
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for ClientSocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        trace!("WS: {:?}", msg);
        match msg {
            ws::Message::Ping(msg) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.heartbeat = Instant::now();
            }
            ws::Message::Text(text) => ctx.text(text),
            ws::Message::Binary(bin) => ctx.binary(bin),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}
