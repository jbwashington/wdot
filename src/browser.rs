use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::captcha::CaptchaSolver;
use crate::config::Config;
use crate::extractor;
use crate::stealth;
use crate::tls;

#[derive(Clone)]
pub struct BrowserPool {
    browser: Arc<Mutex<Browser>>,
    captcha_solver: Option<CaptchaSolver>,
    stealth_enabled: bool,
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
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
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

        // Add stealth-specific launch args
        if config.stealth {
            for arg in stealth::stealth_args() {
                builder = builder.arg(arg);
            }
            // TLS fingerprint evasion args
            for arg in tls::tls_evasion_args() {
                builder = builder.arg(arg);
            }
            // Set a realistic user-agent via Chrome flag
            builder = builder.arg(format!("--user-agent={}", stealth::user_agent()));
        }

        // Proxy support for TLS fingerprint spoofing
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

        let browser_config = builder.build().map_err(|e| format!("Browser config error: {e}"))?;

        let (browser, mut handler) = Browser::launch(browser_config).await?;

        // Spawn the browser event handler
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
        })
    }

    pub async fn fetch(
        &self,
        url: &str,
        wait_for: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<FetchResult, Box<dyn std::error::Error + Send + Sync>> {
        let browser = self.browser.lock().await;
        let page = browser.new_page("about:blank").await?;

        // Apply stealth evasions BEFORE navigating to the target
        if self.stealth_enabled {
            stealth::apply(&page).await?;
        }

        // Now navigate to the target URL
        page.goto(url).await?;
        page.wait_for_navigation_response().await?;

        let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000));

        // If a specific selector was requested, wait for it
        if let Some(selector) = wait_for {
            let _ = tokio::time::timeout(timeout, page.find_element(selector)).await;
        } else {
            // Default: wait for network idle via a short delay
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }

        // Detect and solve captchas if solver is configured
        if let Some(ref solver) = self.captcha_solver {
            solver.detect_and_solve(&page, url).await?;
        }

        // Extract page content
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

        // Close the page to free resources
        page.close().await?;

        // Extract markdown and links
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
