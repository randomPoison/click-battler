use crate::game::*;
use futures::{compat::*, pin_mut, prelude::*, select};
use log::*;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

mod game;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    std::env::set_var("RUST_LOG", "click_battler=debug");
    env_logger::init();

    debug!("test log");

    // Create the game state and spawn the main game loop, keeping the controller
    // handle so that we can pass it to the client tasks that we spawn.
    let handle = GameController::start();

    // GET /chat -> websocket upgrade
    let chat = warp::path("chat")
        // The `ws2()` filter will prepare Websocket handshake...
        .and(warp::ws2())
        .map(move |ws: warp::ws::Ws2| {
            // This will call our function if the handshake succeeds.
            let handle = handle.clone();
            ws.on_upgrade(move |socket| {
                Compat::new(Box::pin(client_connected(socket, handle).map(|_| Ok(()))))
            })
        });

    // GET / -> index html
    let index = warp::fs::dir("static");

    let routes = index.or(chat);

    runtime::spawn(warp::serve(routes).bind(([127, 0, 0, 1], 3030)).compat())
        .await
        .expect("I guess an error happened in the server");
}

async fn client_connected(ws: WebSocket, mut handle: ControllerHandle) {
    debug!("Running client connection logic");

    // Split the socket into a sender and receiver of messages.
    let (mut socket_sender, socket_receiver) = ws.sink_compat().split();

    let (client, client_handle) = Client::new();

    // Allow the game state to handle the newly-connected client.
    let player = handle.client_connected(client_handle).await;
    info!("New client connected, assigned ID {:?}", player.id);

    // Send the client the initial state of the player.
    let player_json = serde_json::to_string(&player).unwrap();
    socket_sender
        .send(Message::text(player_json))
        .await
        .expect("Failed to send client initial player state");

    // Fuse and pin the streams so that we can select over them.
    let update_receiver = client.update.fuse();
    let socket_receiver = socket_receiver.fuse();
    pin_mut!(update_receiver, socket_receiver);

    loop {
        select! {
            update = update_receiver.next() => match update {
                Some(update) => info!("Received message from controller: {:?}", update),
                None => {
                    info!("{:?} update channel dropped, looks like we dead", player.id);
                    break;
                }
            },

            socket_message = socket_receiver.next() => {
                info!("Received message from socket: {:?}", socket_message);
            }

            complete => break,
        }
    }

    unimplemented!("TODO: Handle client disconnected");
}
