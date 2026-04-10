use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub enum AppMode {
    Server,
    Desktop,
}

#[derive(Debug, Clone)]
pub struct TestPlan {
    pub latency_probes: usize,
    pub latency_interval_ms: u64,
    pub throughput_samples: usize,
    pub throughput_workers: usize,
    pub throughput_window_ms: u64,
    pub session_ttl_seconds: u64,
    pub session_start_cooldown_seconds: u64,
    pub max_active_sessions_per_ip: usize,
    pub download_chunk_size: usize,
    pub download_request_size: usize,
    pub upload_payload_size: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub bind_addr: SocketAddr,
    pub browser_host: String,
    pub browser_port: u16,
    pub open_browser_on_start: bool,
    pub autostart_enabled: bool,
    pub node_name: String,
    pub locale: String,
    pub public_dir: PathBuf,
    pub mode: AppMode,
    pub plan: TestPlan,
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Cross-platform speed test server")]
pub struct Cli {
    #[arg(long, default_value = "[::]:3000")]
    pub bind: SocketAddr,

    #[arg(long, default_value = "127.0.0.1")]
    pub browser_host: String,

    #[arg(long)]
    pub browser_port: Option<u16>,

    #[arg(long, default_value_t = true)]
    pub open_browser: bool,

    #[arg(long, default_value_t = false)]
    pub autostart: bool,

    #[arg(long, default_value = "SpeedTest Node")]
    pub node_name: String,

    #[arg(long, default_value = "zh-CN")]
    pub locale: String,

    #[arg(long, default_value = "server")]
    pub mode: String,
}

impl RuntimeConfig {
    pub fn from_cli(cli: Cli, public_dir: PathBuf) -> Self {
        let mode = match cli.mode.as_str() {
            "desktop" => AppMode::Desktop,
            _ => AppMode::Server,
        };

        let browser_port = cli.browser_port.unwrap_or(cli.bind.port());

        Self {
            bind_addr: cli.bind,
            browser_host: cli.browser_host,
            browser_port,
            open_browser_on_start: cli.open_browser,
            autostart_enabled: cli.autostart,
            node_name: cli.node_name,
            locale: cli.locale,
            public_dir,
            mode,
            plan: TestPlan {
                latency_probes: 6,
                latency_interval_ms: 120,
                throughput_samples: 3,
                throughput_workers: 3,
                throughput_window_ms: 900,
                session_ttl_seconds: 30,
                session_start_cooldown_seconds: 5,
                max_active_sessions_per_ip: 2,
                download_chunk_size: 64 * 1024,
                download_request_size: 4 * 1024 * 1024,
                upload_payload_size: 1024 * 1024,
            },
        }
    }

    pub fn browser_url(&self, port: Option<u16>) -> String {
        let port = port.unwrap_or(self.browser_port);
        format!("http://{}:{}/", self.browser_host, port)
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            AppMode::Server => "server",
            AppMode::Desktop => "desktop",
        }
    }

    pub fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.plan.session_ttl_seconds)
    }

    pub fn session_cooldown(&self) -> Duration {
        Duration::from_secs(self.plan.session_start_cooldown_seconds)
    }
}

pub fn default_public_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public")
}
