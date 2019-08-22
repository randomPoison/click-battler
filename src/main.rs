use crate::{client::*, game::*};
use actix::prelude::*;
use actix_files as fs;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use log::*;

mod client;
mod game;

fn main() -> std::io::Result<()> {
    std::env::set_var(
        "RUST_LOG",
        "click_battler=trace,actix_server=info,actix_web=info",
    );
    env_logger::init();

    let game_controller = GameController::new().start();

    HttpServer::new(move || {
        App::new()
            .data(game_controller.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/ws/").route(web::get().to(ws_index)))
            .service(fs::Files::new("/", "static/").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
}

/// Starts a new `ClientSocket` actor for the new client connection.
fn ws_index(
    request: HttpRequest,
    stream: web::Payload,
    controller: web::Data<Addr<GameController>>,
) -> Result<HttpResponse, Error> {
    info!("{:?}", request);

    let response = ws::start(
        ClientSocket::new(controller.get_ref().clone()),
        &request,
        stream,
    );
    info!("{:?}", response.as_ref().unwrap());

    response
}
