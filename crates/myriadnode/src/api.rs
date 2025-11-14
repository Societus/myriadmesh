use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config::ApiConfig;
use crate::failover::FailoverManager;
use crate::heartbeat::HeartbeatService;
use myriadmesh_network::{AdapterManager, AdapterStatus as NetworkAdapterStatus};

/// API server state
#[derive(Clone)]
pub struct ApiState {
    #[allow(dead_code)]
    config: ApiConfig,
    adapter_manager: Arc<RwLock<AdapterManager>>,
    heartbeat_service: Arc<HeartbeatService>,
    failover_manager: Arc<FailoverManager>,
    node_id: String,
    node_name: String,
    start_time: SystemTime,
}

/// API server
pub struct ApiServer {
    config: ApiConfig,
    state: Arc<ApiState>,
}

impl ApiServer {
    pub async fn new(
        config: ApiConfig,
        adapter_manager: Arc<RwLock<AdapterManager>>,
        heartbeat_service: Arc<HeartbeatService>,
        failover_manager: Arc<FailoverManager>,
        node_id: String,
        node_name: String,
    ) -> Result<Self> {
        let state = Arc::new(ApiState {
            config: config.clone(),
            adapter_manager,
            heartbeat_service,
            failover_manager,
            node_id,
            node_name,
            start_time: SystemTime::now(),
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
            // Node endpoints (Web UI expects /api/ prefix)
            .route("/api/node/info", get(get_node_info))
            .route("/api/node/status", get(get_node_status))
            // Adapter endpoints
            .route("/api/adapters", get(list_adapters))
            .route("/api/adapters/:id", get(get_adapter))
            .route("/api/adapters/:id/start", post(start_adapter))
            .route("/api/adapters/:id/stop", post(stop_adapter))
            // Heartbeat endpoints
            .route("/api/heartbeat/stats", get(get_heartbeat_stats))
            .route("/api/heartbeat/nodes", get(get_heartbeat_nodes))
            // Failover endpoints
            .route("/api/failover/events", get(get_failover_events))
            .route("/api/failover/force", post(force_failover))
            // Network config endpoints
            .route("/api/config/network", get(get_network_config))
            .route("/api/config/network", post(update_network_config))
            // i2p endpoints
            .route("/api/i2p/status", get(get_i2p_status))
            .route("/api/i2p/destination", get(get_i2p_destination))
            .route("/api/i2p/tunnels", get(get_i2p_tunnels))
            // Legacy v1 endpoints (for backwards compatibility)
            .route("/api/v1/node/status", get(get_node_status))
            .route("/api/v1/node/info", get(get_node_info))
            .route("/api/v1/messages", post(send_message))
            .route("/api/v1/messages", get(list_messages))
            .route("/api/v1/adapters", get(list_adapters))
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

async fn get_node_info(State(state): State<Arc<ApiState>>) -> Json<NodeInfoResponse> {
    let uptime_secs = state.start_time.elapsed().unwrap_or_default().as_secs();

    Json(NodeInfoResponse {
        node_id: state.node_id.clone(),
        node_name: state.node_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
    })
}

#[derive(Serialize)]
struct NodeInfoResponse {
    node_id: String,
    node_name: String,
    version: String,
    uptime_secs: u64,
}

// === Message Endpoints ===

async fn send_message(
    State(_state): State<Arc<ApiState>>,
    Json(_request): Json<SendMessageRequest>,
) -> Result<Json<MessageResponse>, StatusCode> {
    // TODO: Implement message sending
    Ok(Json(MessageResponse {
        message_id: "msg_123".to_string(),
        status: "queued".to_string(),
    }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SendMessageRequest {
    destination: String,
    payload: String,
    priority: Option<u8>,
}

#[derive(Serialize)]
struct MessageResponse {
    message_id: String,
    status: String,
}

async fn list_messages(State(_state): State<Arc<ApiState>>) -> Json<Vec<MessageInfo>> {
    // TODO: Implement message listing
    Json(vec![])
}

#[derive(Serialize)]
#[allow(dead_code)]
struct MessageInfo {
    message_id: String,
    timestamp: u64,
    direction: String,
    status: String,
}

// === Adapter Endpoints ===

async fn list_adapters(State(state): State<Arc<ApiState>>) -> Json<Vec<AdapterStatus>> {
    let manager = state.adapter_manager.read().await;
    let adapter_ids = manager.adapter_ids();

    let mut statuses = Vec::new();
    for id in adapter_ids {
        if let Some(status) = get_adapter_status_internal(&manager, &id).await {
            statuses.push(status);
        }
    }

    Json(statuses)
}

async fn get_adapter(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<AdapterStatus>, StatusCode> {
    let manager = state.adapter_manager.read().await;

    get_adapter_status_internal(&manager, &id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn start_adapter(
    State(_state): State<Arc<ApiState>>,
    Path(_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // TODO: Implement adapter start
    Ok(StatusCode::OK)
}

async fn stop_adapter(
    State(_state): State<Arc<ApiState>>,
    Path(_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // TODO: Implement adapter stop
    Ok(StatusCode::OK)
}

// Helper function to get adapter status
async fn get_adapter_status_internal(manager: &AdapterManager, id: &str) -> Option<AdapterStatus> {
    // Get adapter
    let adapter_arc = manager.get_adapter(id)?;
    let adapter = adapter_arc.read().await;

    // Get status from adapter
    let network_status = adapter.get_status();

    // Get adapter capabilities
    let capabilities = manager.get_capabilities(id)?;

    let status_str = match network_status {
        NetworkAdapterStatus::Uninitialized => "uninitialized",
        NetworkAdapterStatus::Initializing => "initializing",
        NetworkAdapterStatus::Ready => "ready",
        NetworkAdapterStatus::Unavailable => "unavailable",
        NetworkAdapterStatus::Error => "error",
        NetworkAdapterStatus::ShuttingDown => "shutting_down",
    };

    Some(AdapterStatus {
        id: id.to_string(),
        adapter_type: capabilities.adapter_type.name().to_string(),
        status: status_str.to_string(),
        version: "1.0.0".to_string(), // Default version
        last_reload: 0,
        reload_count: 0,
        reputation_score: 1.0,
        capabilities: vec![],
    })
}

#[derive(Serialize)]
struct AdapterStatus {
    id: String,
    adapter_type: String,
    status: String,
    version: String,
    last_reload: u64,
    reload_count: u32,
    reputation_score: f64,
    capabilities: Vec<String>,
}

// === DHT Endpoints ===

async fn list_dht_nodes(State(_state): State<Arc<ApiState>>) -> Json<Vec<DhtNodeInfo>> {
    // TODO: Get actual DHT node list
    Json(vec![])
}

#[derive(Serialize)]
#[allow(dead_code)]
struct DhtNodeInfo {
    node_id: String,
    last_seen: u64,
    distance: String,
}

// === Heartbeat Endpoints ===

async fn get_heartbeat_stats(State(state): State<Arc<ApiState>>) -> Json<HeartbeatStatsResponse> {
    let stats = state.heartbeat_service.get_stats().await;

    Json(HeartbeatStatsResponse {
        active_nodes: stats.total_nodes,
        total_heartbeats_sent: 0, // TODO: Track heartbeat counts
        total_heartbeats_received: 0,
        average_rtt_ms: 0.0,
    })
}

#[derive(Serialize)]
struct HeartbeatStatsResponse {
    active_nodes: usize,
    total_heartbeats_sent: u64,
    total_heartbeats_received: u64,
    average_rtt_ms: f64,
}

async fn get_heartbeat_nodes(State(state): State<Arc<ApiState>>) -> Json<Vec<HeartbeatNodeEntry>> {
    let node_map = state.heartbeat_service.get_node_map().await;

    let mut entries = Vec::new();
    for (node_id, _node_info) in node_map.iter() {
        entries.push(HeartbeatNodeEntry {
            node_id: node_id.to_hex(),
            status: "alive".to_string(), // TODO: Determine actual status
            last_seen: 0,                // TODO: Get actual last_seen
            rtt_ms: 0.0,                 // TODO: Get actual RTT
            consecutive_failures: 0,     // TODO: Track failures
        });
    }

    // Sort by node_id for now
    entries.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    Json(entries)
}

#[derive(Serialize)]
struct HeartbeatNodeEntry {
    node_id: String,
    status: String,
    last_seen: u64,
    rtt_ms: f64,
    consecutive_failures: u32,
}

// === Failover Endpoints ===

async fn get_failover_events(
    State(_state): State<Arc<ApiState>>,
) -> Json<Vec<FailoverEventResponse>> {
    // TODO: Properly map FailoverEvent enum variants to response format
    Json(vec![])
}

#[derive(Serialize)]
struct FailoverEventResponse {
    timestamp: u64,
    from_adapter: String,
    to_adapter: String,
    reason: String,
    success: bool,
}

async fn force_failover(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ForceFailoverRequest>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "Force failover requested for adapter: {}",
        request.adapter_id
    );

    match state
        .failover_manager
        .force_failover(request.adapter_id)
        .await
    {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Deserialize)]
struct ForceFailoverRequest {
    adapter_id: String,
}

// === Network Config Endpoints ===

async fn get_network_config(State(_state): State<Arc<ApiState>>) -> Json<NetworkConfigResponse> {
    // TODO: Get actual config
    Json(NetworkConfigResponse {
        scoring_mode: "default".to_string(),
        failover_enabled: true,
        heartbeat_enabled: true,
        privacy_mode: false,
    })
}

#[derive(Serialize)]
struct NetworkConfigResponse {
    scoring_mode: String,
    failover_enabled: bool,
    heartbeat_enabled: bool,
    privacy_mode: bool,
}

async fn update_network_config(
    State(_state): State<Arc<ApiState>>,
    Json(_request): Json<UpdateNetworkConfigRequest>,
) -> Result<StatusCode, StatusCode> {
    // TODO: Implement config update
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct UpdateNetworkConfigRequest {
    scoring_mode: Option<String>,
    failover_enabled: Option<bool>,
    heartbeat_enabled: Option<bool>,
    privacy_mode: Option<bool>,
}

// === i2p Endpoints ===

async fn get_i2p_status(State(state): State<Arc<ApiState>>) -> Json<I2pStatusResponse> {
    let manager = state.adapter_manager.read().await;

    // Try to find the i2p adapter
    let i2p_adapter_id = "i2p"; // Standard ID for i2p adapter

    let (router_status, adapter_status, version) =
        if let Some(adapter_arc) = manager.get_adapter(i2p_adapter_id) {
            let adapter = adapter_arc.read().await;
            let status = adapter.get_status();

            let (rs, as_str) = match status {
                NetworkAdapterStatus::Ready => ("running", "ready"),
                NetworkAdapterStatus::Initializing => ("starting", "initializing"),
                NetworkAdapterStatus::Unavailable => ("stopped", "unavailable"),
                NetworkAdapterStatus::Error => ("error", "error"),
                NetworkAdapterStatus::ShuttingDown => ("stopping", "shutting_down"),
                NetworkAdapterStatus::Uninitialized => ("unknown", "uninitialized"),
            };

            (rs, as_str, "1.0.0")
        } else {
            ("unknown", "uninitialized", "unknown")
        };

    Json(I2pStatusResponse {
        router_status: router_status.to_string(),
        adapter_status: adapter_status.to_string(),
        router_version: version.to_string(),
        tunnels_active: 0,  // TODO: Get actual tunnel count
        peers_connected: 0, // TODO: Get actual peer count
    })
}

#[derive(Serialize)]
struct I2pStatusResponse {
    router_status: String,
    adapter_status: String,
    router_version: String,
    tunnels_active: usize,
    peers_connected: usize,
}

async fn get_i2p_destination(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<I2pDestinationResponse>, StatusCode> {
    let _manager = state.adapter_manager.read().await;
    let _i2p_adapter_id = "i2p";

    // TODO: Get actual destination from adapter
    // For now, return a placeholder
    Ok(Json(I2pDestinationResponse {
        destination: "placeholder.b32.i2p".to_string(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        node_id: state.node_id.clone(),
    }))
}

#[derive(Serialize)]
struct I2pDestinationResponse {
    destination: String,
    created_at: u64,
    node_id: String,
}

async fn get_i2p_tunnels(State(_state): State<Arc<ApiState>>) -> Json<I2pTunnelsResponse> {
    // TODO: Get actual tunnel information from i2p adapter
    Json(I2pTunnelsResponse {
        inbound_tunnels: vec![],
        outbound_tunnels: vec![],
        total_bandwidth_bps: 0,
    })
}

#[derive(Serialize)]
struct I2pTunnelsResponse {
    inbound_tunnels: Vec<I2pTunnelInfo>,
    outbound_tunnels: Vec<I2pTunnelInfo>,
    total_bandwidth_bps: u64,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct I2pTunnelInfo {
    tunnel_id: String,
    peers: Vec<String>,
    latency_ms: f64,
    bandwidth_bps: u64,
    status: String,
}
