use crate::game::*;
use derivative::Derivative;
use futures::prelude::*;
use serde::Deserialize;
use warp::filters::ws;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Client {
    game: GameControllerProxy,

    #[derivative(Debug = "ignore")]
    sink: Box<dyn Sink<ws::Message, Error = warp::Error> + Send>,
}

#[thespian::actor]
impl Client {
    pub async fn handle_update(&mut self, update: String) {
        unimplemented!();
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    HealSelf,

    AttackPlayer { target: PlayerId },
}
