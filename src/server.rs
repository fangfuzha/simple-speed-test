use crate::{
    config::{Cli, RuntimeConfig, default_public_dir},
    session::{SessionError, SessionManager},
};
use axum::{
    Json, Router,
    body::Body,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::oneshot};
use tracing::{error, info};

const INDEX_HTML: &str = include_str!("../public/index.html");
const SPEEDTEST_JS: &str = include_str!("../public/speedtest.js");

#[derive(Clone)]
pub struct AppState {
    pub config: RuntimeConfig,
    pub sessions: Arc<SessionManager>,
    pub started_at: Instant,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize)]
struct NodeResponse {
    node_name: String,
    mode: String,
    locale: String,
    uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
struct ConfigResponse {
    node_name: String,
    mode: String,
    browser_url: String,
    test_plan: TestPlanResponse,
}

#[derive(Debug, Serialize)]
struct TestPlanResponse {
    latency_probes: usize,
    latency_interval_ms: u64,
    throughput_samples: usize,
    throughput_workers: usize,
    throughput_window_ms: u64,
    session_ttl_seconds: u64,
    session_start_cooldown_seconds: u64,
    download_request_size: usize,
    upload_payload_size: usize,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    mode: String,
}

#[derive(Debug, Serialize)]
struct PingResponse {
    ok: bool,
    server_time_ms: u128,
}

#[derive(Debug, Serialize)]
struct UploadResponse {
    received: usize,
}

#[derive(Debug, Serialize)]
struct SessionStartResponse {
    token: String,
    expires_in_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct DownloadQuery {
    size: Option<usize>,
}

pub struct ServerHandle {
    pub local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
}

impl ServerHandle {
    pub fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

pub async fn spawn_server(config: RuntimeConfig) -> Result<ServerHandle, std::io::Error> {
    let state = Arc::new(AppState {
        config: config.clone(),
        sessions: Arc::new(SessionManager::new()),
        started_at: Instant::now(),
    });

    let app = build_router(state);
    let listener = TcpListener::bind(config.bind_addr).await?;
    let local_addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    info!("listening on http://{}", local_addr);

    tokio::spawn(async move {
        let result = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await;

        if let Err(err) = result {
            error!(error = %err, "server exited with error");
        }
    });

    Ok(ServerHandle {
        local_addr,
        shutdown: Some(shutdown_tx),
    })
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/speedtest.js", get(speedtest_js))
        .route("/api/health", get(health))
        .route("/api/config", get(config))
        .route("/api/node", get(node))
        .route("/api/session", post(start_session))
        .route("/api/ping", get(ping))
        .route("/api/download", get(download))
        .route("/api/upload", post(upload))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn speedtest_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        SPEEDTEST_JS,
    )
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        mode: state.config.mode_name().to_string(),
    })
}

async fn config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        node_name: state.config.node_name.clone(),
        mode: state.config.mode_name().to_string(),
        browser_url: state.config.browser_url(Some(state.config.browser_port)),
        test_plan: TestPlanResponse {
            latency_probes: state.config.plan.latency_probes,
            latency_interval_ms: state.config.plan.latency_interval_ms,
            throughput_samples: state.config.plan.throughput_samples,
            throughput_workers: state.config.plan.throughput_workers,
            throughput_window_ms: state.config.plan.throughput_window_ms,
            session_ttl_seconds: state.config.plan.session_ttl_seconds,
            session_start_cooldown_seconds: state.config.plan.session_start_cooldown_seconds,
            download_request_size: state.config.plan.download_request_size,
            upload_payload_size: state.config.plan.upload_payload_size,
        },
    })
}

async fn node(State(state): State<Arc<AppState>>) -> Json<NodeResponse> {
    Json(NodeResponse {
        node_name: state.config.node_name.clone(),
        mode: state.config.mode_name().to_string(),
        locale: state.config.locale.clone(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

async fn start_session(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<SessionStartResponse>, ApiError> {
    let ip = addr.ip();
    match state.sessions.create_session(ip, &state.config) {
        Ok(session) => Ok(Json(SessionStartResponse {
            token: session.token,
            expires_in_seconds: session.expires_in_seconds,
        })),
        Err(SessionError::RateLimited) => Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "同一公网 IP 的测速启动过于频繁，请稍后再试。",
        )),
        Err(SessionError::TooManyActive) => Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "当前公网 IP 的活动测速会话过多，请稍后再试。",
        )),
        Err(SessionError::Invalid) => {
            Err(ApiError::new(StatusCode::UNAUTHORIZED, "测速会话无效。"))
        }
    }
}

async fn ping(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<PingResponse>, ApiError> {
    validate_session(&state, addr.ip(), &headers)?;
    Ok(Json(PingResponse {
        ok: true,
        server_time_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    }))
}

async fn download(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, ApiError> {
    validate_session(&state, addr.ip(), &headers)?;

    let size = query
        .size
        .unwrap_or(state.config.plan.download_request_size)
        .min(8 * 1024 * 1024)
        .max(64 * 1024);
    let chunk_size = state.config.plan.download_chunk_size.max(1024);

    let stream = async_stream::stream! {
        let mut sent = 0usize;
        while sent < size {
            let remaining = size - sent;
            let current = remaining.min(chunk_size);
            sent += current;
            yield Ok::<Bytes, std::io::Error>(Bytes::from(vec![b'a'; current]));
        }
    };

    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&size.to_string()) {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    Ok(response)
}

async fn upload(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResponse>, ApiError> {
    validate_session(&state, addr.ip(), &headers)?;
    Ok(Json(UploadResponse {
        received: body.len(),
    }))
}

fn validate_session(
    state: &Arc<AppState>,
    ip: IpAddr,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let token = headers
        .get("x-speedtest-session")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "缺少测速会话令牌。"))?;

    if state.sessions.validate_session(ip, token) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "测速会话已过期或无效。",
        ))
    }
}

pub async fn run_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    let public_dir = default_public_dir();
    let config = RuntimeConfig::from_cli(cli, public_dir);

    let handle = spawn_server(config.clone()).await?;
    let browser_url = config.browser_url(Some(handle.local_addr.port()));
    info!("Server is running at {}", browser_url);
    info!("Press Ctrl+C to stop the server.");
    tokio::signal::ctrl_c().await?;
    handle.stop();
    Ok(())
}

#[cfg(feature = "desktop")]
pub async fn run_desktop() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::ErrorKind;

    let cli = Cli::parse();
    let public_dir = default_public_dir();
    let mut config = RuntimeConfig::from_cli(cli, public_dir);

    let mut settings = crate::settings::DesktopSettings::load().unwrap_or_default();
    if settings.autostart != config.autostart_enabled {
        settings.autostart = config.autostart_enabled;
    }
    if settings.open_browser_on_start != config.open_browser_on_start {
        settings.open_browser_on_start = config.open_browser_on_start;
    }
    let _ = settings.save();
    let _ = crate::tray::register_autostart(&settings);

    let handle = match spawn_server(config.clone()).await {
        Ok(handle) => handle,
        Err(err) if err.kind() == ErrorKind::AddrInUse => {
            let original_port = config.bind_addr.port();
            config.bind_addr.set_port(0);
            let handle = match spawn_server(config.clone()).await {
                Ok(handle) => {
                    let message = format!(
                        "端口 {} 已被占用，已自动切换到随机端口 {}。",
                        original_port,
                        handle.local_addr.port()
                    );
                    let _ = rfd::MessageDialog::new()
                        .set_title("端口已占用")
                        .set_description(&message)
                        .set_buttons(rfd::MessageButtons::Ok)
                        .set_level(rfd::MessageLevel::Info)
                        .show();
                    handle
                }
                Err(err) => {
                    let message = format!("无法绑定端口 {}: {}", original_port, err);
                    let _ = rfd::MessageDialog::new()
                        .set_title("测速程序启动失败")
                        .set_description(&message)
                        .set_buttons(rfd::MessageButtons::Ok)
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                    return Err(err.into());
                }
            };
            handle
        }
        Err(err) => {
            let message = format!("无法绑定端口 {}: {}", config.bind_addr, err);
            let _ = rfd::MessageDialog::new()
                .set_title("测速程序启动失败")
                .set_description(&message)
                .set_buttons(rfd::MessageButtons::Ok)
                .set_level(rfd::MessageLevel::Error)
                .show();
            return Err(err.into());
        }
    };
    let browser_url = config.browser_url(Some(handle.local_addr.port()));

    if settings.open_browser_on_start {
        let _ = webbrowser::open(&browser_url);
    }

    let tray_state = crate::tray::TrayState::new(settings, config, browser_url.clone(), handle);
    tray_state.run()?;
    Ok(())
}
