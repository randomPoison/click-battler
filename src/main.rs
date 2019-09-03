#![recursion_limit = "256"]

use crate::game::*;
use futures::{compat::*, pin_mut, prelude::*, select};
use log::*;
use std::net::SocketAddr;
use thespian::Actor;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

mod client;
mod game;

static TEST_ADDR: &str = "127.0.0.1:3030";
static RELEASE_ADDR: &str = "0.0.0.0:3030";

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    std::env::set_var("RUST_LOG", "click_battler=debug");
    env_logger::init();

    // Create the game state and spawn the main game loop, keeping the controller
    // handle so that we can pass it to the client tasks that we spawn.
    let handle = GameController::default().spawn();

    // GET /chat -> websocket upgrade
    let chat = warp::path("chat")
        .and(warp::ws2())
        .map(move |ws: warp::ws::Ws2| {
            let handle = handle.clone();
            ws.on_upgrade(move |socket| {
                let fut = async move {
                    handle.client_connected(socket);
                    Ok(())
                };
                Compat::new(fut.boxed())
            })
        });

    // GET / -> index html
    let index = warp::fs::dir("static");

    let routes = index.or(chat);

    let addr: SocketAddr = RELEASE_ADDR.parse().unwrap();
    runtime::spawn(warp::serve(routes).bind(addr).compat())
        .await
        .expect("I guess an error happened in the server");
}
