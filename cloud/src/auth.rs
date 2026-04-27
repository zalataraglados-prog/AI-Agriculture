use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

const DEFAULT_AUTH_USERNAME: &str = "admin";
const DEFAULT_AUTH_PASSWORD: &str = "admin123!";
const DEFAULT_AUTH_TTL_SEC: u64 = 8 * 60 * 60;

#[derive(Debug, Clone)]
pub(crate) struct AuthConfig {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) token_ttl_sec: u64,
}

impl AuthConfig {
    pub(crate) fn from_env() -> Self {
        let username = std::env::var("CLOUD_AUTH_USERNAME")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_AUTH_USERNAME.to_string());
        let password = std::env::var("CLOUD_AUTH_PASSWORD")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_AUTH_PASSWORD.to_string());
        let token_ttl_sec = std::env::var("CLOUD_AUTH_TTL_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_AUTH_TTL_SEC);

        Self {
            username,
            password,
            token_ttl_sec,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AuthSession {
    pub(crate) username: String,
    pub(crate) token: String,
    pub(crate) issued_at_epoch_sec: u64,
    pub(crate) expires_at_epoch_sec: u64,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    username: String,
    issued_at_epoch_sec: u64,
    expires_at_epoch_sec: u64,
}

#[derive(Debug)]
pub(crate) struct AuthManager {
    cfg: AuthConfig,
    sessions: HashMap<String, SessionRecord>,
}

impl AuthManager {
    pub(crate) fn from_config(cfg: AuthConfig) -> Self {
        Self {
            cfg,
            sessions: HashMap::new(),
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::from_config(AuthConfig::from_env())
    }

    pub(crate) fn login(&mut self, username: &str, password: &str) -> Result<AuthSession, String> {
        self.cleanup_expired();
        if username.trim() != self.cfg.username || password != self.cfg.password {
            return Err("invalid username or password".to_string());
        }

        let now = now_epoch_sec();
        let issued = now;
        let expires = now.saturating_add(self.cfg.token_ttl_sec);
        let token = generate_token(48);
        let rec = SessionRecord {
            username: self.cfg.username.clone(),
            issued_at_epoch_sec: issued,
            expires_at_epoch_sec: expires,
        };
        self.sessions.insert(token.clone(), rec.clone());
        Ok(AuthSession {
            username: rec.username,
            token,
            issued_at_epoch_sec: issued,
            expires_at_epoch_sec: expires,
        })
    }

    pub(crate) fn validate(&mut self, token: &str) -> Option<AuthSession> {
        self.cleanup_expired();
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        let rec = self.sessions.get(token)?;
        Some(AuthSession {
            username: rec.username.clone(),
            token: token.to_string(),
            issued_at_epoch_sec: rec.issued_at_epoch_sec,
            expires_at_epoch_sec: rec.expires_at_epoch_sec,
        })
    }

    pub(crate) fn logout(&mut self, token: &str) -> bool {
        self.cleanup_expired();
        self.sessions.remove(token.trim()).is_some()
    }

    fn cleanup_expired(&mut self) {
        let now = now_epoch_sec();
        self.sessions
            .retain(|_, rec| rec.expires_at_epoch_sec > now);
    }
}

fn generate_token(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn now_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{AuthConfig, AuthManager};

    #[test]
    fn login_and_validate_session_token() {
        let mut auth = AuthManager::from_config(AuthConfig {
            username: "u".to_string(),
            password: "p".to_string(),
            token_ttl_sec: 3600,
        });
        let session = auth.login("u", "p").expect("login should pass");
        let validated = auth.validate(&session.token).expect("must validate");
        assert_eq!(validated.username, "u");
    }

    #[test]
    fn wrong_password_is_rejected() {
        let mut auth = AuthManager::from_config(AuthConfig {
            username: "u".to_string(),
            password: "p".to_string(),
            token_ttl_sec: 3600,
        });
        let err = auth.login("u", "x").expect_err("must reject");
        assert!(err.contains("invalid"));
    }

    #[test]
    fn logout_invalidates_token() {
        let mut auth = AuthManager::from_config(AuthConfig {
            username: "u".to_string(),
            password: "p".to_string(),
            token_ttl_sec: 3600,
        });
        let session = auth.login("u", "p").expect("login should pass");
        assert!(auth.logout(&session.token));
        assert!(auth.validate(&session.token).is_none());
    }

    #[test]
    fn expired_session_is_pruned() {
        let mut auth = AuthManager::from_config(AuthConfig {
            username: "u".to_string(),
            password: "p".to_string(),
            token_ttl_sec: 1,
        });
        let session = auth.login("u", "p").expect("login should pass");
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(auth.validate(&session.token).is_none());
    }
}
