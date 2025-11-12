use anyhow::Result;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config::ApiConfig;

/// API server state
#[derive(Clone)]
pub struct ApiState {
    config: ApiConfig,
}

/// API server
pub struct ApiServer {
    config: ApiConfig,
    state: Arc<ApiState>,
}

impl ApiServer {
    pub async fn new(config: ApiConfig) -> Result<Self> {
        let state = Arc::new(ApiState {
            config: config.clone(),
        });

        Ok(Self { config, state })
    }

    pub async fn start(&self) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let app = self.create_router();

        let bind_addr = format!("{}:{}", self.config.bind, self.config.port);
        let listener = TcpListener::bind(&bind_addr).await?;

        info!("API server listening on {}", bind_addr);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e))
        });

        Ok(handle)
    }

    fn create_router(&self) -> Router {
        Router::new()
            // Health check
            .route("/health", get(health_check))
            // Node endpoints
            .route("/api/v1/node/status", get(get_node_status))
            .route("/api/v1/node/info", get(get_node_info))
            // Message endpoints
            .route("/api/v1/messages", post(send_message))
            .route("/api/v1/messages", get(list_messages))
            // Adapter endpoints
            .route("/api/v1/adapters", get(list_adapters))
            // DHT endpoints
            .route("/api/v1/dht/nodes", get(list_dht_nodes))
            // Add CORS middleware
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone())
    }
}

// === Health Check ===

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

// === Node Endpoints ===

async fn get_node_status(State(_state): State<Arc<ApiState>>) -> Json<NodeStatusResponse> {
    Json(NodeStatusResponse {
        status: "running".to_string(),
        uptime_secs: 0, // TODO: Calculate actual uptime
        adapters_active: 0,
        messages_queued: 0,
    })
}

#[derive(Serialize)]
struct NodeStatusResponse {
    status: String,
    uptime_secs: u64,
    adapters_active: usize,
    messages_queued: usize,
}

async fn get_node_info(State(_state): State<Arc<ApiState>>) -> Json<NodeInfoResponse> {
    Json(NodeInfoResponse {
        node_id: "placeholder".to_string(), // TODO: Get from state
        name: "myriadnode".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Serialize)]
struct NodeInfoResponse {
    node_id: String,
    name: String,
    version: String,
}

// === Message Endpoints ===

async fn send_message(
    State(_state): State<Arc<ApiState>>,
    Json(_payload): Json<SendMessageRequest>,
) -> Json<SendMessageResponse> {
    // TODO: Implement message sending
    Json(SendMessageResponse {
        message_id: uuid::Uuid::new_v4().to_string(),
        status: "queued".to_string(),
    })
}

#[derive(Deserialize)]
struct SendMessageRequest {
    destination: String,
    payload: String,
    priority: Option<String>,
}

#[derive(Serialize)]
struct SendMessageResponse {
    message_id: String,
    status: String,
}

async fn list_messages(State(_state): State<Arc<ApiState>>) -> Json<Vec<MessageInfo>> {
    // TODO: Implement message listing
    Json(vec![])
}

#[derive(Serialize)]
struct MessageInfo {
    id: String,
    destination: String,
    status: String,
    created_at: String,
}

// === Adapter Endpoints ===

async fn list_adapters(State(_state): State<Arc<ApiState>>) -> Json<Vec<AdapterInfo>> {
    // TODO: Get actual adapter list from network manager
    Json(vec![
        AdapterInfo {
            name: "ethernet".to_string(),
            adapter_type: "Ethernet".to_string(),
            status: "active".to_string(),
            enabled: true,
        },
    ])
}

#[derive(Serialize)]
struct AdapterInfo {
    name: String,
    adapter_type: String,
    status: String,
    enabled: bool,
}

// === DHT Endpoints ===

async fn list_dht_nodes(State(_state): State<Arc<ApiState>>) -> Json<Vec<DhtNodeInfo>> {
    // TODO: Get actual DHT node list
    Json(vec![])
}

#[derive(Serialize)]
struct DhtNodeInfo {
    node_id: String,
    adapters: Vec<String>,
    last_seen: String,
}
