use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tokio::sync::Mutex;
use tracing::warn;

pub enum ProxyDirection {
    ClientToServer,
    ServerToClient,
}

#[derive(Clone, Debug)]
pub enum ProxyState {
    /// Number of running connection
    Connected(u64),
    /// Timestamp_ms from last connection
    Disconnected(tokio::time::Instant),
    NoConnectionEstablished,
}
impl ProxyState {
    pub fn active_connections(&self) -> u64 {
        match self {
            ProxyState::Connected(num) => *num,
            _ => 0,
        }
    }
}
#[derive(Clone)]
pub struct Connections {
    state: Arc<Mutex<ProxyState>>,
    num_connections: Arc<AtomicI64>,
    client_to_server_bytes: Arc<AtomicI64>,
    server_to_client_bytes: Arc<AtomicI64>,
}
#[derive(Clone, Debug)]
pub struct Metrics {
    pub state: ProxyState,
    pub num_connections: u64,
    pub client_to_server_bytes: u64,
    pub server_to_client_bytes: u64,
}

impl Drop for Connections {
    fn drop(&mut self) {
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut state = state.lock().await;
            *state = match &*state {
                ProxyState::Connected(1) => ProxyState::Disconnected(tokio::time::Instant::now()),
                ProxyState::Connected(num) if *num > 1 => ProxyState::Connected(num - 1),
                ProxyState::Disconnected(_)
                | ProxyState::Connected(_)
                | ProxyState::NoConnectionEstablished => {
                    warn!("Invalid state");
                    ProxyState::Disconnected(tokio::time::Instant::now())
                }
            };
        });
    }
}

impl Connections {
    pub(crate) fn new() -> Self {
        Connections {
            state: Arc::new(Mutex::new(ProxyState::NoConnectionEstablished)),
            num_connections: Arc::new(AtomicI64::new(0)),
            client_to_server_bytes: Arc::new(AtomicI64::new(0)),
            server_to_client_bytes: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn message(&self, direction: ProxyDirection, bytes: usize) {
        match direction {
            ProxyDirection::ClientToServer => self
                .client_to_server_bytes
                .fetch_add(bytes as i64, Ordering::Relaxed),
            ProxyDirection::ServerToClient => self
                .server_to_client_bytes
                .fetch_add(bytes as i64, Ordering::Relaxed),
        };
    }
    pub(crate) async fn new_connection(&self) -> Self {
        let mut state = self.state.lock().await;
        self.num_connections.fetch_add(1, Ordering::Relaxed);
        let connections = Connections {
            state: self.state.clone(),
            num_connections: self.num_connections.clone(),
            client_to_server_bytes: self.client_to_server_bytes.clone(),
            server_to_client_bytes: self.server_to_client_bytes.clone(),
        };
        *state = match &*state {
            ProxyState::Connected(num) => ProxyState::Connected(num + 1),
            ProxyState::Disconnected(_) | ProxyState::NoConnectionEstablished => {
                ProxyState::Connected(1)
            }
        };
        connections
    }
    pub async fn metrics(&self) -> Metrics {
        let state = self.state.lock().await;
        Metrics {
            state: (*state).clone(),
            num_connections: self.num_connections.load(Ordering::Relaxed) as u64,
            client_to_server_bytes: self.client_to_server_bytes.load(Ordering::Relaxed) as u64,
            server_to_client_bytes: self.server_to_client_bytes.load(Ordering::Relaxed) as u64,
        }
    }
}
