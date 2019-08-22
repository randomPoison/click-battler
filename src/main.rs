extern crate futures;
extern crate pretty_env_logger;
extern crate warp;

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use futures::channel::mpsc;
use futures::compat::*;
use futures::prelude::*;
use warp::ws::{Message, WebSocket};
use warp::Filter;

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    pretty_env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Arc::new(Mutex::new(HashMap::new()));
    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

    // GET /chat -> websocket upgrade
    let chat = warp::path("chat")
        // The `ws2()` filter will prepare Websocket handshake...
        .and(warp::ws2())
        .and(users)
        .map(|ws: warp::ws::Ws2, users| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| Compat::new(Box::pin(user_connected(socket, users))))
        });

    // GET / -> index html
    let index = warp::fs::dir("static");

    let routes = index.or(chat);

    runtime::spawn(warp::serve(routes).bind(([127, 0, 0, 1], 3030)).compat())
        .await
        .expect("I guess an error happened in the server");
}

async fn user_connected(ws: WebSocket, users: Users) -> Result<(), ()> {
    // Use a counter to assign a new unique ID for this user.
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new chat user: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.sink_compat().split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, mut rx) = mpsc::unbounded();
    runtime::spawn(async move {
        while let Some(item) = rx.next().await {
            if let Err(err) = user_ws_tx.send(item).await {
                eprintln!("websocket send error: {}", err);
            }
        }
    });

    // Save the sender in our list of connected users.
    users.lock().unwrap().insert(my_id, tx);

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    // Make an extra clone to give to our disconnection handler...
    let users2 = users.clone();

    while let Some(msg_result) = user_ws_rx.next().await {
        match msg_result {
            Ok(msg) => user_message(my_id, msg, &users),
            Err(err) => eprintln!("websocket error(uid={}): {}", my_id, err),
        }
    }
    user_disconnected(my_id, &users2);

    Ok(())
}

fn user_message(my_id: usize, msg: Message, users: &Users) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("<User#{}>: {}", my_id, msg);

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    for (&uid, tx) in users.lock().unwrap().iter() {
        if my_id != uid {
            match tx.unbounded_send(Message::text(new_msg.clone())) {
                Ok(()) => (),
                Err(_disconnected) => {
                    // The tx is disconnected, our `user_disconnected` code
                    // should be happening in another task, nothing more to
                    // do here.
                }
            }
        }
    }
}

fn user_disconnected(my_id: usize, users: &Users) {
    eprintln!("good bye user: {}", my_id);

    // Stream closed up, so remove from the user list
    users.lock().unwrap().remove(&my_id);
}
