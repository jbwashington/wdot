use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub twocaptcha_api_key: Option<String>,
    pub chrome_path: Option<String>,
    pub headless: bool,
    pub proxy_url: Option<String>,
    pub stealth: bool,
    pub data_dir: PathBuf,
    pub behavior_profile: Option<String>,
    pub reputation_enabled: bool,
    pub reputation_window: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let home = env::var("HOME").unwrap_or_else(|_| ".".into());
        let default_data_dir = PathBuf::from(&home).join(".wdot");

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
            data_dir: env::var("WDOT_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(default_data_dir),
            behavior_profile: env::var("BEHAVIOR_PROFILE").ok().filter(|s| !s.is_empty()),
            reputation_enabled: env::var("REPUTATION")
                .map(|v| v != "false")
                .unwrap_or(true),
            reputation_window: env::var("REPUTATION_WINDOW")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
        }
    }
}
