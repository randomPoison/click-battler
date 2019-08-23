use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
    select,
};
use log::*;
use runtime::time::Interval;
use std::{collections::HashMap, sync::atomic::*, time::Duration};

#[derive(Debug)]
pub struct GameController {
    players: HashMap<PlayerId, Player>,
    id_counter: AtomicU64,
}

impl GameController {
    pub fn start() -> ControllerHandle {
        let (client_connected_sender, client_connected_receiver) = mpsc::unbounded();

        let mut controller = GameController {
            players: HashMap::new(),
            id_counter: AtomicU64::new(0),
        };

        let handle = ControllerHandle {
            client_connected: client_connected_sender,
        };

        runtime::spawn(async move {
            let mut health_tick = Interval::new(Duration::from_secs(1)).fuse();
            let mut client_connected = client_connected_receiver.fuse();

            loop {
                select! {
                    _ = health_tick.next() => controller.tick_player_health(),
                    result_sender = client_connected.next() => {
                        let result = controller.handle_client_connected();
                        result_sender.unwrap().send(result).expect("Failed to send result of client connected");
                    }
                }
            }
        });

        handle
    }

    fn next_player_id(&self) -> PlayerId {
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        PlayerId(id)
    }

    fn tick_player_health(&mut self) {
        trace!("Ticking player health");

        let mut dead_players = Vec::new();

        // Every tick, reduce player health by 1. Keep track of any players that died as a result.
        for player in self.players.values_mut() {
            player.health -= 1;

            if player.health == 0 {
                dead_players.push(player.id);
            }
        }

        // Notify all connected clients of the dead player.

        // At the end of the frame, remove any players that have died and notify the clients.
        for id in dead_players.drain(..) {
            self.players.remove(&id);

            // TODO: Broadcast to clients that a player died.
        }
    }

    fn handle_client_connected(&mut self) -> PlayerHandle {
        let id = self.next_player_id();
        let (update_sender, update_receiver) = mpsc::channel(10);

        info!("New client connected, assigning ID: {:?}", id);

        // Create a new player for the client and add it to the set of players.
        let player = Player {
            id,
            health: 10,
            update: update_sender,
        };
        self.players.insert(id, player);

        PlayerHandle {
            id,
            update: update_receiver,
        }
    }
}

/// A handle to the game controller, exposing functionality for communicating
/// with the controller asynchronously.
#[derive(Debug, Clone)]
pub struct ControllerHandle {
    client_connected: mpsc::UnboundedSender<oneshot::Sender<PlayerHandle>>,
}

impl ControllerHandle {
    pub async fn client_connected(&mut self) -> PlayerHandle {
        let (result_sender, result_receiver) = oneshot::channel();

        // Send the message to the client controller.
        self.client_connected
            .send(result_sender)
            .await
            .expect("Failed to send client connected message to controller");

        // Wait for the controller to respond with the result.
        result_receiver
            .await
            .expect("Failed to receive player ID for new client connection")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(u64);

#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,

    // The player's current health.
    pub health: u32,

    pub update: mpsc::Sender<()>,
}

#[derive(Debug)]
pub struct PlayerHandle {
    pub id: PlayerId,
    pub update: mpsc::Receiver<()>,
}
