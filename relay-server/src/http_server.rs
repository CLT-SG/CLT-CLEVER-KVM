//! HTTP Server for Web Dashboard and KVM Client
//!
//! Uses Actix Web to serve the web dashboard, KVM client, and REST API.
//! Templates are rendered using Tera templating engine.

use crate::device::{DeviceRegistry, DeviceSummary, StreamState};
use crate::ws_relay::WsRelayState;
use actix_files::Files;
use actix_web::{
    get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result as ActixResult,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tera::{Context, Tera};
use tracing::{error, info};

// ============================================================================
// Template Engine - Embedded Templates
// ============================================================================

// Embed templates at compile time so they work regardless of working directory
const BASE_TEMPLATE: &str = include_str!("../templates/base.html");
const DASHBOARD_TEMPLATE: &str = include_str!("../templates/dashboard.html");
const KVM_CLIENT_TEMPLATE: &str = include_str!("../templates/kvm_client.html");
const ERROR_TEMPLATE: &str = include_str!("../templates/error.html");

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        
        // Add embedded templates
        tera.add_raw_template("base.html", BASE_TEMPLATE)
            .expect("Failed to add base.html template");
        tera.add_raw_template("dashboard.html", DASHBOARD_TEMPLATE)
            .expect("Failed to add dashboard.html template");
        tera.add_raw_template("kvm_client.html", KVM_CLIENT_TEMPLATE)
            .expect("Failed to add kvm_client.html template");
        tera.add_raw_template("error.html", ERROR_TEMPLATE)
            .expect("Failed to add error.html template");
        
        tera.autoescape_on(vec![".html"]);
        tera
    };
}

// ============================================================================
// Application State
// ============================================================================

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<DeviceRegistry>,
    pub ws_relay: Arc<WsRelayState>,
    pub server_hostname: String,
    pub server_port: u16,
}

// ============================================================================
// View Models
// ============================================================================

/// Device view model for templates
#[derive(Serialize)]
struct DeviceViewModel {
    hostname: String,
    display_name: String,
    os: String,
    is_online: bool,
    online_class: String,
    state_class: String,
    state_text: String,
    viewer_count: usize,
    stream_config: StreamConfigViewModel,
}

#[derive(Serialize)]
struct StreamConfigViewModel {
    width: u32,
    height: u32,
    fps: u32,
    codec: String,
}

impl From<&DeviceSummary> for DeviceViewModel {
    fn from(d: &DeviceSummary) -> Self {
        let (state_class, state_text) = match &d.state {
            StreamState::Idle => ("idle", "Idle"),
            StreamState::Streaming => ("streaming", "Streaming"),
            StreamState::Paused => ("paused", "Paused"),
            StreamState::Error(e) => ("error", e.as_str()),
        };

        Self {
            hostname: d.hostname.clone(),
            display_name: d.display_name.clone(),
            os: d.os.clone(),
            is_online: d.is_online,
            online_class: if d.is_online { "online" } else { "offline" }.to_string(),
            state_class: state_class.to_string(),
            state_text: state_text.to_string(),
            viewer_count: d.viewer_count as usize,
            stream_config: StreamConfigViewModel {
                width: d.stream_config.width,
                height: d.stream_config.height,
                fps: d.stream_config.fps,
                codec: d.stream_config.codec.clone(),
            },
        }
    }
}

// ============================================================================
// Dashboard Routes
// ============================================================================

/// Dashboard page handler
#[get("/")]
async fn dashboard_handler(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let devices = data.registry.list_devices().await;
    let stats = data.registry.get_stats().await;

    let device_vms: Vec<DeviceViewModel> = devices.iter().map(DeviceViewModel::from).collect();

    let mut context = Context::new();
    context.insert("server_hostname", &data.server_hostname);
    context.insert("server_port", &data.server_port);
    context.insert("devices", &device_vms);
    context.insert("stats", &stats);

    match TEMPLATES.render("dashboard.html", &context) {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => {
            error!("Template error: {}", e);
            Ok(HttpResponse::InternalServerError().body("Template rendering failed"))
        }
    }
}

/// Alias for /dashboard
#[get("/dashboard")]
async fn dashboard_alias_handler(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let devices = data.registry.list_devices().await;
    let stats = data.registry.get_stats().await;

    let device_vms: Vec<DeviceViewModel> = devices.iter().map(DeviceViewModel::from).collect();

    let mut context = Context::new();
    context.insert("server_hostname", &data.server_hostname);
    context.insert("server_port", &data.server_port);
    context.insert("devices", &device_vms);
    context.insert("stats", &stats);

    match TEMPLATES.render("dashboard.html", &context) {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => {
            error!("Template error: {}", e);
            Ok(HttpResponse::InternalServerError().body("Template rendering failed"))
        }
    }
}

// ============================================================================
// KVM Client Routes
// ============================================================================

#[derive(Deserialize)]
pub struct KvmQueryParams {
    hostname: String,
    #[serde(default)]
    audio: bool,
    #[serde(default = "default_codec")]
    codec: String,
}

fn default_codec() -> String {
    "h264".to_string()
}

/// KVM client page handler
#[get("/kvm")]
async fn kvm_client_handler(
    data: web::Data<AppState>,
    query: web::Query<KvmQueryParams>,
) -> ActixResult<HttpResponse> {
    let hostname = query.hostname.to_lowercase();

    match data.registry.get_device(&hostname).await {
        Some(device) => {
            if !device.is_online(30) {
                return render_error(
                    "Device Offline",
                    &format!("The device '{}' is currently offline.", hostname),
                );
            }

            let mut context = Context::new();
            context.insert("server_hostname", &data.server_hostname);
            context.insert("server_port", &data.server_port);
            context.insert("device_hostname", &device.hostname);
            context.insert("device_name", &device.display_name);
            context.insert("width", &device.stream_config.width);
            context.insert("height", &device.stream_config.height);
            context.insert("audio_enabled", &query.audio);
            context.insert("codec", &query.codec);

            match TEMPLATES.render("kvm_client.html", &context) {
                Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
                Err(e) => {
                    error!("Template error: {}", e);
                    Ok(HttpResponse::InternalServerError().body("Template rendering failed"))
                }
            }
        }
        None => render_error(
            "Device Not Found",
            &format!("No device with hostname '{}' is registered.", hostname),
        ),
    }
}

/// Render error page
fn render_error(title: &str, message: &str) -> ActixResult<HttpResponse> {
    let mut context = Context::new();
    context.insert("title", title);
    context.insert("message", message);

    match TEMPLATES.render("error.html", &context) {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => {
            error!("Template error: {}", e);
            Ok(HttpResponse::InternalServerError().body("Error page rendering failed"))
        }
    }
}

// ============================================================================
// API Routes
// ============================================================================

/// List all devices
#[get("/api/devices")]
async fn list_devices_handler(data: web::Data<AppState>) -> impl Responder {
    let devices = data.registry.list_devices().await;
    web::Json(devices)
}

/// Get a specific device
#[get("/api/devices/{hostname}")]
async fn get_device_handler(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let hostname = path.into_inner();
    match data.registry.get_device(&hostname).await {
        Some(device) => Ok(HttpResponse::Ok().json(DeviceSummary::from(&device))),
        None => Ok(HttpResponse::NotFound().json(ApiError {
            error: "Device not found".to_string(),
        })),
    }
}

/// Get viewers for a device
#[get("/api/devices/{hostname}/viewers")]
async fn get_device_viewers_handler(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let hostname = path.into_inner();
    let viewers = data.registry.get_viewers_for_device(&hostname).await;
    let viewer_infos: Vec<ViewerInfo> = viewers
        .iter()
        .map(|v| ViewerInfo {
            id: v.id.clone(),
            connected_at: v.connected_at.to_rfc3339(),
        })
        .collect();
    web::Json(viewer_infos)
}

#[derive(Serialize)]
struct ViewerInfo {
    id: String,
    connected_at: String,
}

#[derive(Serialize)]
struct ApiError {
    error: String,
}

/// Get registry statistics
#[get("/api/stats")]
async fn stats_handler(data: web::Data<AppState>) -> impl Responder {
    let stats = data.registry.get_stats().await;
    web::Json(stats)
}

/// Health check endpoint
#[get("/api/health")]
async fn health_handler() -> impl Responder {
    web::Json(HealthResponse {
        status: "ok".to_string(),
        version: "2.0.0".to_string(),
    })
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

// ============================================================================
// Device Registration Routes
// ============================================================================

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub hostname: String,
    pub display_name: Option<String>,
    pub ip_address: String,
    pub ws_port: u16,
    pub capabilities: Option<crate::device::DeviceCapabilities>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub device_id: String,
    pub message: String,
}

/// Register a device
#[post("/api/register")]
async fn register_device_handler(
    data: web::Data<AppState>,
    req: web::Json<RegisterRequest>,
) -> impl Responder {
    use crate::device::Device;

    let mut device = Device::new(req.hostname.clone(), req.ip_address.clone(), req.ws_port);

    if let Some(ref name) = req.display_name {
        device.display_name = name.clone();
    }

    if let Some(ref caps) = req.capabilities {
        device.capabilities = caps.clone();
    }

    let device = data.registry.register_device(device).await;

    info!(
        "📱 Device registered: {} ({})",
        device.display_name, device.hostname
    );

    web::Json(RegisterResponse {
        success: true,
        device_id: device.id,
        message: format!("Device '{}' registered successfully", device.hostname),
    })
}

/// Device heartbeat
#[post("/api/heartbeat/{hostname}")]
async fn heartbeat_handler(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let hostname = path.into_inner();
    if data.registry.heartbeat(&hostname).await {
        Ok(HttpResponse::Ok().json(HeartbeatResponse { success: true }))
    } else {
        Ok(HttpResponse::NotFound().json(ApiError {
            error: "Device not found".to_string(),
        }))
    }
}

#[derive(Serialize)]
struct HeartbeatResponse {
    success: bool,
}

/// Unregister a device
#[post("/api/unregister/{hostname}")]
async fn unregister_handler(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let hostname = path.into_inner();
    data.registry.unregister_device(&hostname).await;
    info!("📱 Device unregistered: {}", hostname);

    web::Json(UnregisterResponse {
        success: true,
        message: format!("Device '{}' unregistered", hostname),
    })
}

#[derive(Serialize)]
struct UnregisterResponse {
    success: bool,
    message: String,
}

// ============================================================================
// WebSocket Routes
// ============================================================================

/// WebSocket handler for device connections
#[get("/ws/device/{hostname}")]
async fn device_ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let hostname = path.into_inner();
    info!("🔌 Device WebSocket connection request: {}", hostname);

    let (response, session, stream) = actix_ws::handle(&req, stream)?;

    actix_rt::spawn(crate::ws_relay::handle_device_connection(
        session,
        stream,
        hostname,
        data.registry.clone(),
        data.ws_relay.clone(),
    ));

    Ok(response)
}

/// WebSocket handler for viewer connections
#[get("/ws/viewer/{hostname}")]
async fn viewer_ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let hostname = path.into_inner();
    info!("👁️ Viewer WebSocket connection request for: {}", hostname);

    let (response, session, stream) = actix_ws::handle(&req, stream)?;

    actix_rt::spawn(crate::ws_relay::handle_viewer_connection(
        session,
        stream,
        hostname,
        data.registry.clone(),
        data.ws_relay.clone(),
    ));

    Ok(response)
}

// ============================================================================
// Server Configuration
// ============================================================================

/// Configure and return the Actix Web application
pub fn configure_app(cfg: &mut web::ServiceConfig, state: AppState) {
    cfg.app_data(web::Data::new(state))
        // Dashboard routes
        .service(dashboard_handler)
        .service(dashboard_alias_handler)
        // KVM client route
        .service(kvm_client_handler)
        // API routes
        .service(list_devices_handler)
        .service(get_device_handler)
        .service(get_device_viewers_handler)
        .service(stats_handler)
        .service(health_handler)
        // Device registration routes
        .service(register_device_handler)
        .service(heartbeat_handler)
        .service(unregister_handler)
        // WebSocket routes
        .service(device_ws_handler)
        .service(viewer_ws_handler)
        // Static files
        .service(Files::new("/static", "static").show_files_listing());
}

/// Create and run the HTTP server
pub async fn run_server(state: AppState, bind_addr: &str) -> std::io::Result<()> {
    let state_clone = state.clone();

    HttpServer::new(move || {
        App::new()
            .configure(|cfg| configure_app(cfg, state_clone.clone()))
            .wrap(actix_web::middleware::Logger::default())
            .wrap(
                actix_web::middleware::DefaultHeaders::new()
                    .add(("Access-Control-Allow-Origin", "*"))
                    .add(("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
                    .add(("Access-Control-Allow-Headers", "Content-Type")),
            )
    })
    .bind(bind_addr)?
    .run()
    .await
}
