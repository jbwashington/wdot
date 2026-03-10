use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::behavior::BehaviorEngine;
use crate::captcha::CaptchaSolver;
use crate::config::Config;
use crate::extractor;
use crate::reputation::signals::{detect_block_signals, SessionSignals};
use crate::reputation::ReputationMonitor;
use crate::stealth;
use crate::tls;

#[derive(Clone)]
pub struct BrowserPool {
    browser: Arc<Mutex<Browser>>,
    captcha_solver: Option<CaptchaSolver>,
    stealth_enabled: bool,
    reputation: Option<Arc<ReputationMonitor>>,
    behavior: Option<Arc<BehaviorEngine>>,
}

#[derive(Debug)]
pub struct FetchResult {
    pub url: String,
    pub title: String,
    pub markdown: String,
    pub raw_html: String,
    pub links: Vec<Link>,
}

#[derive(Debug, serde::Serialize)]
pub struct Link {
    pub text: String,
    pub href: String,
}

impl BrowserPool {
    pub async fn new(
        config: &Config,
        reputation: Option<Arc<ReputationMonitor>>,
        behavior: Option<Arc<BehaviorEngine>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = BrowserConfig::builder()
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-background-networking")
            .arg("--disable-default-apps")
            .arg("--disable-sync")
            .arg("--disable-translate")
            .arg("--metrics-recording-only")
            .arg("--mute-audio")
            .arg("--no-first-run");

        if config.stealth {
            for arg in stealth::stealth_args() {
                builder = builder.arg(arg);
            }
            for arg in tls::tls_evasion_args() {
                builder = builder.arg(arg);
            }
            builder = builder.arg(format!("--user-agent={}", stealth::user_agent()));
        }

        if let Some(ref proxy_url) = config.proxy_url {
            builder = builder.arg(tls::proxy_arg(proxy_url));
            for arg in tls::proxy_cert_args() {
                builder = builder.arg(arg);
            }
        }

        if config.headless {
            builder = builder.arg("--headless=new");
        }

        if let Some(ref path) = config.chrome_path {
            builder = builder.chrome_executable(path);
        }

        let browser_config = builder
            .build()
            .map_err(|e| format!("Browser config error: {e}"))?;

        let (browser, mut handler) = Browser::launch(browser_config).await?;

        tokio::spawn(async move {
            while let Some(_event) = handler.next().await {}
        });

        let captcha_solver = config
            .twocaptcha_api_key
            .as_ref()
            .map(|key| CaptchaSolver::new(key.clone()));

        Ok(Self {
            browser: Arc::new(Mutex::new(browser)),
            captcha_solver,
            stealth_enabled: config.stealth,
            reputation,
            behavior,
        })
    }

    pub async fn fetch(
        &self,
        url: &str,
        wait_for: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<FetchResult, Box<dyn std::error::Error + Send + Sync>> {
        // Check if reputation monitor says we should pause
        if let Some(ref rep) = self.reputation {
            if rep.is_paused().await {
                tracing::warn!("Reputation critical — cooling down for 60s");
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            // Apply adaptive delay
            let delay = rep.current_delay_ms().await;
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }

        // Apply behavior pre-navigation delay
        if let Some(ref beh) = self.behavior {
            if let Some(mut replayer) = beh.replayer().await {
                replayer.pre_navigate_delay().await;
            }
        }

        let start = Instant::now();
        let browser = self.browser.lock().await;
        let page = browser.new_page("about:blank").await?;

        if self.stealth_enabled {
            stealth::apply(&page).await?;
        }

        page.goto(url).await?;
        page.wait_for_navigation_response().await?;

        let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000));

        if let Some(selector) = wait_for {
            let _ = tokio::time::timeout(timeout, page.find_element(selector)).await;
        } else {
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }

        // Simulate human reading behavior
        if let Some(ref beh) = self.behavior {
            if let Some(mut replayer) = beh.replayer().await {
                replayer.simulate_reading(&page).await;
            }
        }

        // Detect and solve captchas
        if let Some(ref solver) = self.captcha_solver {
            solver.detect_and_solve(&page, url).await?;
        }

        let html = page
            .evaluate("document.documentElement.outerHTML")
            .await?
            .into_value::<String>()?;

        let title = page
            .evaluate("document.title")
            .await?
            .into_value::<String>()
            .unwrap_or_default();

        let current_url = page
            .evaluate("window.location.href")
            .await?
            .into_value::<String>()
            .unwrap_or_else(|_| url.to_string());

        page.close().await?;

        let response_time = start.elapsed().as_millis() as u64;

        // Record reputation signals
        if let Some(ref rep) = self.reputation {
            let (blocked, challenge, rate_limited) = detect_block_signals(&html, 200);
            let captcha_encountered = html.contains("g-recaptcha")
                || html.contains("h-captcha")
                || html.contains("cf-turnstile")
                || html.contains("challenges.cloudflare.com");

            let domain = extract_domain(url);

            rep.record(SessionSignals {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                domain,
                captcha_encountered,
                captcha_type: None,
                http_status: 200,
                blocked,
                redirect_to_challenge: challenge,
                response_time_ms: response_time,
                tls_fingerprint_hash: "default".into(),
                rate_limited,
            })
            .await;
        }

        let markdown = extractor::html_to_markdown(&html, None);
        let links = extractor::extract_links(&html);

        Ok(FetchResult {
            url: current_url,
            title,
            markdown,
            raw_html: html,
            links,
        })
    }
}

fn extract_domain(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .split(':')
        .next()
        .unwrap_or(url)
        .to_string()
}
