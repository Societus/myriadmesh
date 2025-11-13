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
        id: state.node_id.clone(),
        name: state.node_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
    })
}

#[derive(Serialize)]
struct NodeInfoResponse {
    id: String,
    name: String,
    version: String,
    uptime_secs: u64,
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
#[allow(dead_code)]
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

async fn list_adapters(State(state): State<Arc<ApiState>>) -> Json<Vec<AdapterStatus>> {
    let manager = state.adapter_manager.read().await;
    let adapter_ids = manager.adapter_ids();

    let mut adapters = Vec::new();
    for adapter_id in adapter_ids {
        if let Some(adapter_status) =
            get_adapter_status_internal(&manager, adapter_id.as_str()).await
        {
            adapters.push(adapter_status);
        }
    }

    Json(adapters)
}

async fn get_adapter(
    State(state): State<Arc<ApiState>>,
    Path(adapter_id): Path<String>,
) -> Result<Json<AdapterStatus>, StatusCode> {
    let manager = state.adapter_manager.read().await;

    if let Some(adapter_status) = get_adapter_status_internal(&manager, &adapter_id).await {
        Ok(Json(adapter_status))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn start_adapter(
    State(state): State<Arc<ApiState>>,
    Path(adapter_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let manager = state.adapter_manager.read().await;

    if let Some(adapter) = manager.get_adapter(&adapter_id) {
        let mut adapter_lock = adapter.write().await;
        match adapter_lock.start().await {
            Ok(_) => Ok(StatusCode::OK),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn stop_adapter(
    State(state): State<Arc<ApiState>>,
    Path(adapter_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let manager = state.adapter_manager.read().await;

    if let Some(adapter) = manager.get_adapter(&adapter_id) {
        let mut adapter_lock = adapter.write().await;
        match adapter_lock.stop().await {
            Ok(_) => Ok(StatusCode::OK),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// Helper function to get adapter status
async fn get_adapter_status_internal(
    manager: &AdapterManager,
    adapter_id: &str,
) -> Option<AdapterStatus> {
    let capabilities = manager.get_capabilities(adapter_id)?;
    let metrics = manager.get_metrics(adapter_id)?;

    // Check if adapter is active by getting the adapter and checking its status
    let adapter_opt = manager.get_adapter(adapter_id);
    let active = if let Some(adapter_arc) = adapter_opt {
        let adapter = adapter_arc.read().await;
        matches!(adapter.get_status(), NetworkAdapterStatus::Ready)
    } else {
        false
    };

    Some(AdapterStatus {
        adapter_id: adapter_id.to_string(),
        adapter_type: format!("{:?}", capabilities.adapter_type),
        active,
        is_backhaul: false, // TODO: Implement backhaul detection query
        health_status: "Healthy".to_string(), // TODO: Get from failover manager
        metrics: AdapterMetrics {
            latency_ms: metrics.latency_ms,
            bandwidth_bps: metrics.bandwidth_bps,
            reliability: metrics.reliability,
            power_consumption: estimate_power_consumption(&capabilities.power_consumption),
            privacy_level: estimate_privacy_level(adapter_id),
        },
    })
}

fn estimate_power_consumption(power: &myriadmesh_network::types::PowerConsumption) -> f64 {
    match power {
        myriadmesh_network::types::PowerConsumption::None => 0.0,
        myriadmesh_network::types::PowerConsumption::VeryLow => 0.1,
        myriadmesh_network::types::PowerConsumption::Low => 0.3,
        myriadmesh_network::types::PowerConsumption::Medium => 0.5,
        myriadmesh_network::types::PowerConsumption::High => 0.7,
        myriadmesh_network::types::PowerConsumption::VeryHigh => 0.9,
    }
}

fn estimate_privacy_level(adapter_id: &str) -> f64 {
    if adapter_id.contains("i2p") {
        0.95
    } else if adapter_id.contains("bluetooth") && !adapter_id.contains("_le") {
        0.85
    } else if adapter_id.contains("bluetooth_le") {
        0.70
    } else if adapter_id.contains("ethernet") || adapter_id.contains("wifi") {
        0.15
    } else if adapter_id.contains("cellular") {
        0.10
    } else {
        0.50
    }
}

#[derive(Serialize)]
struct AdapterStatus {
    adapter_id: String,
    adapter_type: String,
    active: bool,
    is_backhaul: bool,
    health_status: String,
    metrics: AdapterMetrics,
}

#[derive(Serialize)]
struct AdapterMetrics {
    latency_ms: f64,
    bandwidth_bps: u64,
    reliability: f64,
    power_consumption: f64,
    privacy_level: f64,
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

// === Heartbeat Endpoints ===

async fn get_heartbeat_stats(State(state): State<Arc<ApiState>>) -> Json<HeartbeatStatsResponse> {
    let stats = state.heartbeat_service.get_stats().await;

    Json(HeartbeatStatsResponse {
        total_nodes: stats.total_nodes,
        nodes_with_location: stats.nodes_with_location,
        adapter_counts: stats.adapter_counts,
    })
}

#[derive(Serialize)]
struct HeartbeatStatsResponse {
    total_nodes: usize,
    nodes_with_location: usize,
    adapter_counts: std::collections::HashMap<String, usize>,
}

async fn get_heartbeat_nodes(State(state): State<Arc<ApiState>>) -> Json<Vec<HeartbeatNodeEntry>> {
    let node_map = state.heartbeat_service.get_node_map().await;

    let entries: Vec<HeartbeatNodeEntry> = node_map
        .into_iter()
        .map(|(node_id, node_info)| {
            let adapters = node_info
                .adapters
                .into_iter()
                .map(|a| HeartbeatAdapterInfo {
                    adapter_id: a.adapter_id,
                    adapter_type: a.adapter_type,
                    active: a.active,
                    bandwidth_bps: a.bandwidth_bps,
                    latency_ms: a.latency_ms,
                    privacy_level: a.privacy_level,
                })
                .collect();

            HeartbeatNodeEntry {
                node_id: format!("{:?}", node_id),
                last_seen: node_info.last_seen,
                adapters,
                heartbeat_count: node_info.heartbeat_count,
            }
        })
        .collect();

    Json(entries)
}

#[derive(Serialize)]
struct HeartbeatNodeEntry {
    node_id: String,
    last_seen: u64,
    adapters: Vec<HeartbeatAdapterInfo>,
    heartbeat_count: u64,
}

#[derive(Serialize)]
struct HeartbeatAdapterInfo {
    adapter_id: String,
    adapter_type: String,
    active: bool,
    bandwidth_bps: u64,
    latency_ms: u32,
    privacy_level: f64,
}

// === Failover Endpoints ===

async fn get_failover_events(
    State(state): State<Arc<ApiState>>,
) -> Json<Vec<FailoverEventResponse>> {
    let events = state.failover_manager.get_recent_events(100).await;

    let responses: Vec<FailoverEventResponse> = events
        .into_iter()
        .map(|event| {
            let (event_type, details) = match event {
                crate::failover::FailoverEvent::AdapterSwitch { from, to, reason } => (
                    "AdapterSwitch".to_string(),
                    format!("Switched from {} to {} ({})", from, to, reason),
                ),
                crate::failover::FailoverEvent::ThresholdViolation {
                    adapter,
                    metric,
                    value,
                    threshold,
                } => (
                    "ThresholdViolation".to_string(),
                    format!(
                        "{}: {} exceeded threshold {} (current: {})",
                        adapter, metric, threshold, value
                    ),
                ),
                crate::failover::FailoverEvent::AdapterDown { adapter, reason } => (
                    "AdapterDown".to_string(),
                    format!("{} went down ({})", adapter, reason),
                ),
                crate::failover::FailoverEvent::AdapterRecovered { adapter } => (
                    "AdapterRecovered".to_string(),
                    format!("{} recovered", adapter),
                ),
            };

            FailoverEventResponse {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                event_type,
                details,
            }
        })
        .collect();

    Json(responses)
}

#[derive(Serialize)]
struct FailoverEventResponse {
    timestamp: u64,
    event_type: String,
    details: String,
}

async fn force_failover(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ForceFailoverRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!(
        "Force failover requested to adapter: {}",
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
