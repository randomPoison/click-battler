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
    trace!("Entering client controller task");

    // Split the socket into a sender and receiver of messages.
    let (mut socket_sender, socket_receiver) = ws.sink_compat().split();

    let (client, client_handle) = Client::new();

    // Allow the game state to handle the newly-connected client.
    let (player_id, init_state) = handle.client_connected(client_handle).await;
    info!("New client connected, assigned ID {:?}", player_id);

    // Send the client the initial state of the player.
    socket_sender
        .send(Message::text(init_state))
        .await
        .expect("Failed to send client initial player state");

    // Fuse and pin the streams so that we can select over them.
    let update_receiver = client.update.fuse();
    let socket_receiver = socket_receiver.fuse();
    pin_mut!(update_receiver, socket_receiver);

    loop {
        select! {
            update = update_receiver.next() => match update {
                Some(update) => {
                    // NOTE: We discard the result here because this will only fail if we have
                    // disconnected from the socket, which we will handle the next time we attempt
                    // to receive from the socket.
                    let _ = socket_sender.send(Message::text(update)).await;
                }

                None => panic!("Client {:?} lost connection with game controller", player_id),
            },

            socket_message = socket_receiver.next() => match socket_message {
                Some(socket_message) => info!("Received message from socket: {:?}", socket_message),
                None => {
                    info!("{:?} socket disconnected", player_id);
                    break;
                }
            }
        }
    }

    // TODO: Notify the game controller that the client has disconnected.

    trace!("Exiting client controller task");
}
