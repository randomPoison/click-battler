use crate::game::*;
use futures::{select, pin_mut, channel::mpsc, compat::*, prelude::*};
use log::*;
use std::sync::{Arc, Mutex};
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

mod game;

// TODO: Don't pass around references to the state, use channels to communicate with it.
type State = Arc<Mutex<GameController>>;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    std::env::set_var(
        "RUST_LOG",
        "click_battler=trace",
    );
    env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Arc::new(Mutex::new(GameController::new()));
    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

    // GET /chat -> websocket upgrade
    let chat = warp::path("chat")
        // The `ws2()` filter will prepare Websocket handshake...
        .and(warp::ws2())
        .and(users)
        .map(|ws: warp::ws::Ws2, users| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| Compat::new(Box::pin(user_connected(socket, users).map(|_| Ok(())))))
        });

    // GET / -> index html
    let index = warp::fs::dir("static");

    let routes = index.or(chat);

    runtime::spawn(warp::serve(routes).bind(([127, 0, 0, 1], 3030)).compat())
        .await
        .expect("I guess an error happened in the server");
}

async fn user_connected(ws: WebSocket, state: State) {
    // Create a channel for receiving messages from the server.
    let (server_sender, mut server_receiver) = mpsc::unbounded();

    // Split the socket into a sender and receiver of messages.
    let (mut socket_sender, mut socket_receiver) = ws.sink_compat().split();

    // Allow the game state to handle the newly-connected client.
    let player_id = state.lock().unwrap().handle_client_connected(server_sender);
    info!("New client connected, assigned ID {:?}", player_id);

    let server_receiver = server_receiver.fuse();
    let socket_receiver = socket_receiver.fuse();

    pin_mut!(server_receiver, socket_receiver);

    loop {
        select! {
            server_message = server_receiver.next() => {
                info!("Received message from server: {:?}", server_message);
            }

            socket_message = socket_receiver.next() => {
                info!("Received message from socket: {:?}", socket_message);
            }

            complete => break,
        }
    }

    // Stream closed up, so remove from the user list.
    state.lock().unwrap().handle_client_disconnected(player_id);
}
