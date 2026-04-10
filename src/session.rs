use crate::config::RuntimeConfig;
use serde::Serialize;
use std::{collections::HashMap, net::IpAddr, sync::Mutex, time::Instant};

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub ip: IpAddr,
    pub expires_at: Instant,
    pub last_seen: Instant,
}

#[derive(Default)]
struct SessionStore {
    sessions: HashMap<String, SessionInfo>,
    last_start_by_ip: HashMap<IpAddr, Instant>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug)]
pub enum SessionError {
    RateLimited,
    TooManyActive,
    Invalid,
}

pub struct SessionManager {
    store: Mutex<SessionStore>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(SessionStore::default()),
        }
    }

    pub fn create_session(
        &self,
        ip: IpAddr,
        config: &RuntimeConfig,
    ) -> Result<SessionResponse, SessionError> {
        let mut store = self.store.lock().expect("session store poisoned");
        self.cleanup_locked(&mut store);

        if let Some(last_start) = store.last_start_by_ip.get(&ip) {
            if last_start.elapsed() < config.session_cooldown() {
                return Err(SessionError::RateLimited);
            }
        }

        let active_count = store
            .sessions
            .values()
            .filter(|session| session.ip == ip)
            .count();
        if active_count >= config.plan.max_active_sessions_per_ip {
            return Err(SessionError::TooManyActive);
        }

        let now = Instant::now();
        let token = new_token();
        store.last_start_by_ip.insert(ip, now);
        store.sessions.insert(
            token.clone(),
            SessionInfo {
                ip,
                expires_at: now + config.session_ttl(),
                last_seen: now,
            },
        );

        Ok(SessionResponse {
            token,
            expires_in_seconds: config.plan.session_ttl_seconds,
        })
    }

    pub fn validate_session(&self, ip: IpAddr, token: &str) -> bool {
        let mut store = self.store.lock().expect("session store poisoned");
        self.cleanup_locked(&mut store);

        if let Some(session) = store.sessions.get_mut(token) {
            if session.ip == ip && session.expires_at > Instant::now() {
                session.last_seen = Instant::now();
                return true;
            }
        }

        false
    }

    fn cleanup_locked(&self, store: &mut SessionStore) {
        let now = Instant::now();
        store.sessions.retain(|_, session| session.expires_at > now);
        store
            .last_start_by_ip
            .retain(|_, instant| instant.elapsed().as_secs() < 60);
    }
}

fn new_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos, counter)
}
