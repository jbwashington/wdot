use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub twocaptcha_api_key: Option<String>,
    pub chrome_path: Option<String>,
    pub headless: bool,
    /// Optional proxy URL for TLS fingerprint evasion (e.g., curl-impersonate proxy).
    pub proxy_url: Option<String>,
    /// Enable stealth mode (default: true).
    pub stealth: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3100),
            twocaptcha_api_key: env::var("TWOCAPTCHA_API_KEY").ok().filter(|s| !s.is_empty()),
            chrome_path: env::var("CHROME_PATH").ok().filter(|s| !s.is_empty()),
            headless: env::var("HEADLESS")
                .map(|v| v != "false")
                .unwrap_or(true),
            proxy_url: env::var("PROXY_URL").ok().filter(|s| !s.is_empty()),
            stealth: env::var("STEALTH")
                .map(|v| v != "false")
                .unwrap_or(true),
        }
    }
}
