use futures::channel::mpsc::UnboundedSender;
use log::*;
use std::{collections::HashMap, sync::atomic::*};
use warp::ws;

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

    pub fn next_player_id(&self) -> PlayerId {
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        PlayerId(id)
    }

    pub fn handle_client_connected(&mut self, sender: UnboundedSender<ws::Message>) -> PlayerId {
        // Create a new player for the client and add it to the set of players.
        let id = self.next_player_id();
        let player = Player { id, health: 10 };
        self.players.insert(id, player);

        id
    }

    pub fn handle_client_disconnected(&mut self, id: PlayerId) {
        // TODO: Maybe do something about that?
        info!("good bye user: {:?}", id);
    }

    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        self.players.get(&id)
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut Player> {
        self.players.get_mut(&id)
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
