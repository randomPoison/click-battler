use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
    select,
};
use log::*;
use runtime::time::Interval;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::atomic::*, time::Duration};

/// A channel that takes parameters and returns a response.
type ResponseChannel<Params, Response> = mpsc::UnboundedSender<(Params, oneshot::Sender<Response>)>;

#[derive(Debug)]
pub struct GameController {
    /// The set of connected clients. Used to broadcast updates to the game state.
    clients: HashMap<PlayerId, ClientHandle>,

    /// State data for all players.
    players: HashMap<PlayerId, Player>,

    id_counter: AtomicU64,
}

impl GameController {
    pub fn start() -> ControllerHandle {
        let (client_connected_sender, client_connected_receiver) = mpsc::unbounded();
        let (client_disconnected_sender, client_disconnected_receiver) = mpsc::unbounded();

        let mut controller = GameController {
            clients: HashMap::new(),
            players: HashMap::new(),
            id_counter: AtomicU64::new(0),
        };

        let handle = ControllerHandle {
            client_connected: client_connected_sender,
            client_disconnected: client_disconnected_sender,
        };

        runtime::spawn(async move {
            let mut health_tick = Interval::new(Duration::from_secs(1)).fuse();
            let mut client_connected = client_connected_receiver.fuse();
            let mut client_disconnected = client_disconnected_receiver.fuse();

            loop {
                select! {
                    _ = health_tick.next() => controller.tick_player_health().await,

                    message = client_connected.next() => {
                        let (client_handle, result_sender) = message.expect("Lost connection with new client channel");
                        let result = controller.handle_client_connected(client_handle).await;
                        result_sender.send(result).expect("Failed to send result of client connected");
                    }

                    message = client_disconnected.next() => {
                        let id = message.expect("Lost connection with client channel");
                        controller.client_disconnected(id);
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

    async fn tick_player_health(&mut self) {
        trace!("Ticking player health");

        let mut dead_players = Vec::new();

        // Every tick, reduce player health by 1. Keep track of any players that died as a result.
        for player in self.players.values_mut() {
            player.health -= 1;

            if player.health == 0 {
                dead_players.push(player.id);
            }
        }

        // Broadcast updated health to all players.
        broadcast_update(
            self.clients.values_mut(),
            GameUpdate::WorldUpdate {
                players: &self.players,
            },
        )
        .await;

        // At the end of the frame, remove any players that have died and notify the clients.
        for id in dead_players.drain(..) {
            self.players.remove(&id);

            broadcast_update(self.clients.values_mut(), GameUpdate::PlayerDied { id }).await;
        }
    }

    async fn handle_client_connected(&mut self, client_handle: ClientHandle) -> (PlayerId, String) {
        let id = self.next_player_id();
        info!("New client connected, assigning ID: {:?}", id);

        let player = Player { id, health: 10 };
        self.players.insert(id, player.clone());

        // Broadcast the new player to any existing clients.
        broadcast_update(
            self.clients.values_mut(),
            GameUpdate::PlayerJoined { player },
        )
        .await;

        // Create a new player for the client and add it to the set of players.
        self.clients.insert(id, client_handle);

        // Return the ID of the new player and the current state of the world to the client.
        let world_state = serde_json::to_string(&self.players).unwrap();
        (id, world_state)
    }

    fn client_disconnected(&mut self, id: PlayerId) {
        trace!("Removing client connection for {:?}", id);
        self.clients.remove(&id);
    }
}

/// A handle to the game controller, exposing functionality for communicating
/// with the controller asynchronously.
#[derive(Debug, Clone)]
pub struct ControllerHandle {
    client_connected: ResponseChannel<ClientHandle, (PlayerId, String)>,
    client_disconnected: mpsc::UnboundedSender<PlayerId>,
}

impl ControllerHandle {
    /// Creates a new client connection, and returns the initial state of the player created for the client.
    pub async fn client_connected(&mut self, client: ClientHandle) -> (PlayerId, String) {
        let (result_sender, result_receiver) = oneshot::channel();

        // Send the message to the client controller.
        self.client_connected
            .send((client, result_sender))
            .await
            .expect("Lost connection to game controller");

        // Wait for the controller to respond with the result.
        result_receiver
            .await
            .expect("Lost connection to game controller")
    }

    pub async fn client_disconnected(&mut self, client: PlayerId) {
        self.client_disconnected
            .send(client)
            .await
            .expect("Lost connection to game controller");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlayerId(u64);

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub id: PlayerId,

    // The player's current health.
    pub health: u32,
}

#[derive(Debug)]
pub struct Client {
    pub update: mpsc::Receiver<String>,
}

impl Client {
    pub fn new() -> (Self, ClientHandle) {
        let (update_sender, update_receiver) = mpsc::channel(10);

        let client = Client {
            update: update_receiver,
        };
        let handle = ClientHandle {
            update: update_sender,
        };

        (client, handle)
    }
}

#[derive(Debug, Clone)]
pub struct ClientHandle {
    update: mpsc::Sender<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum GameUpdate<'a> {
    PlayerJoined {
        player: Player,
    },

    PlayerDied {
        id: PlayerId,
    },

    WorldUpdate {
        players: &'a HashMap<PlayerId, Player>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    HealSelf,

    AttackPlayer { target: PlayerId },
}

async fn broadcast_update(
    clients: impl Iterator<Item = &mut ClientHandle>,
    update: GameUpdate<'_>,
) {
    // Serialize the update so that we can send it.
    let update = serde_json::to_string(&update).unwrap();

    // TODO: Send all updates concurrently.
    for client in clients {
        // NOTE: We discard the result here because we will handle disconnected
        // clients elsewhere.
        let _ = client.update.send(update.clone()).await;
    }
}
