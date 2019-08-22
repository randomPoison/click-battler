use actix::prelude::*;
use actix_web_actors::ws;
use log::*;
use std::{collections::HashMap, sync::atomic::*};

#[derive(Debug)]
pub struct GameController {
    players: HashMap<PlayerId, Player>,
    id_counter: AtomicU64,
}

impl GameController {
    pub fn new() -> Self {
        GameController {
            players: HashMap::new(),
            id_counter: AtomicU64::new(0),
        }
    }

    pub fn generate_player_id(&self) -> PlayerId {
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        PlayerId(id)
    }
}

impl Actor for GameController {
    type Context = Context<Self>;
}

impl Handler<ClientConnected> for GameController {
    type Result = MessageResult<ClientConnected>;

    fn handle(&mut self, _message: ClientConnected, _ctx: &mut Self::Context) -> Self::Result {
        // Create a new player for the client and add it to the set of players.
        let id = self.generate_player_id();
        let player = Player {
            id,
            health: 10,
        };
        self.players.insert(id, player);

        MessageResult(CreatePlayer(id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(u64);

#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,

    // The player's current health.
    pub health: u32,
}

#[derive(Debug, Message)]
#[rtype(CreatePlayer)]
pub struct ClientConnected;

#[derive(Debug, Message)]
pub struct CreatePlayer(pub PlayerId);
