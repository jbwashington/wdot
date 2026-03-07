use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct CaptchaSolver {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug)]
enum CaptchaType {
    ReCaptchaV2 { sitekey: String },
    ReCaptchaV3 { sitekey: String, action: String },
    HCaptcha { sitekey: String },
    Turnstile { sitekey: String, action: Option<String>, data: Option<String> },
}

// --- 2Captcha API v2 types ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskRequest {
    client_key: String,
    task: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskResponse {
    error_id: u32,
    #[serde(default)]
    task_id: Option<u64>,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetTaskResultRequest {
    client_key: String,
    task_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetTaskResultResponse {
    error_id: u32,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    solution: Option<serde_json::Value>,
    #[serde(default)]
    error_code: Option<String>,
}

impl CaptchaSolver {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Detect captchas on the page and solve them automatically.
    pub async fn detect_and_solve(
        &self,
        page: &Page,
        page_url: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let captcha = self.detect_captcha(page).await?;
        let Some(captcha) = captcha else {
            return Ok(());
        };

        tracing::info!("Captcha detected: {:?}", captcha);

        match captcha {
            CaptchaType::ReCaptchaV2 { sitekey } => {
                let token = self.solve_recaptcha_v2(&sitekey, page_url).await?;
                self.inject_recaptcha_token(page, &token).await?;
            }
            CaptchaType::ReCaptchaV3 { sitekey, action } => {
                let token = self.solve_recaptcha_v3(&sitekey, page_url, &action).await?;
                self.inject_recaptcha_token(page, &token).await?;
            }
            CaptchaType::HCaptcha { sitekey } => {
                let token = self.solve_hcaptcha(&sitekey, page_url).await?;
                self.inject_hcaptcha_token(page, &token).await?;
            }
            CaptchaType::Turnstile { sitekey, action, data } => {
                let token = self.solve_turnstile(&sitekey, page_url, action.as_deref(), data.as_deref()).await?;
                self.inject_turnstile_token(page, &token).await?;
            }
        }

        // Wait for page to process the captcha solution
        tokio::time::sleep(Duration::from_millis(2000)).await;

        Ok(())
    }

    async fn detect_captcha(
        &self,
        page: &Page,
    ) -> Result<Option<CaptchaType>, Box<dyn std::error::Error + Send + Sync>> {
        // Check for Cloudflare Turnstile first (most common blocker)
        let turnstile_info = page
            .evaluate(
                r#"(() => {
                    // Explicit Turnstile widget
                    const el = document.querySelector('.cf-turnstile[data-sitekey]');
                    if (el) return JSON.stringify({
                        sitekey: el.getAttribute('data-sitekey'),
                        action: el.getAttribute('data-action') || null,
                        cdata: el.getAttribute('data-cdata') || null,
                    });
                    // Turnstile via script tag
                    const script = document.querySelector('script[src*="challenges.cloudflare.com"]');
                    if (script) {
                        const widget = document.querySelector('[data-sitekey]');
                        if (widget) return JSON.stringify({
                            sitekey: widget.getAttribute('data-sitekey'),
                            action: widget.getAttribute('data-action') || null,
                            cdata: widget.getAttribute('data-cdata') || null,
                        });
                    }
                    // Turnstile managed challenge (full-page)
                    const iframe = document.querySelector('iframe[src*="challenges.cloudflare.com"]');
                    if (iframe) {
                        const match = iframe.src.match(/sitekey=([^&]+)/);
                        if (match) return JSON.stringify({ sitekey: match[1], action: 'managed', cdata: null });
                    }
                    return null;
                })()"#,
            )
            .await?
            .into_value::<Option<String>>()
            .unwrap_or(None);

        if let Some(info_str) = turnstile_info {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&info_str) {
                return Ok(Some(CaptchaType::Turnstile {
                    sitekey: info["sitekey"].as_str().unwrap_or_default().to_string(),
                    action: info["action"].as_str().map(|s| s.to_string()),
                    data: info["cdata"].as_str().map(|s| s.to_string()),
                }));
            }
        }

        // Check for reCAPTCHA v2
        let recaptcha_v2_sitekey = page
            .evaluate(
                r#"(() => {
                    const el = document.querySelector('.g-recaptcha[data-sitekey]');
                    return el ? el.getAttribute('data-sitekey') : null;
                })()"#,
            )
            .await?
            .into_value::<Option<String>>()
            .unwrap_or(None);

        if let Some(sitekey) = recaptcha_v2_sitekey {
            return Ok(Some(CaptchaType::ReCaptchaV2 { sitekey }));
        }

        // Check for reCAPTCHA v3
        let recaptcha_v3_sitekey = page
            .evaluate(
                r#"(() => {
                    const scripts = Array.from(document.querySelectorAll('script[src*="recaptcha"]'));
                    for (const s of scripts) {
                        const match = s.src.match(/render=([^&]+)/);
                        if (match && match[1] !== 'explicit') return match[1];
                    }
                    return null;
                })()"#,
            )
            .await?
            .into_value::<Option<String>>()
            .unwrap_or(None);

        if let Some(sitekey) = recaptcha_v3_sitekey {
            return Ok(Some(CaptchaType::ReCaptchaV3 {
                sitekey,
                action: "verify".into(),
            }));
        }

        // Check for hCaptcha
        let hcaptcha_sitekey = page
            .evaluate(
                r#"(() => {
                    const el = document.querySelector('.h-captcha[data-sitekey]');
                    return el ? el.getAttribute('data-sitekey') : null;
                })()"#,
            )
            .await?
            .into_value::<Option<String>>()
            .unwrap_or(None);

        if let Some(sitekey) = hcaptcha_sitekey {
            return Ok(Some(CaptchaType::HCaptcha { sitekey }));
        }

        Ok(None)
    }

    // --- 2Captcha API v2 solvers ---

    async fn solve_recaptcha_v2(
        &self,
        sitekey: &str,
        page_url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let task = serde_json::json!({
            "type": "RecaptchaV2TaskProxyless",
            "websiteURL": page_url,
            "websiteKey": sitekey,
        });
        let solution = self.create_and_poll(task).await?;
        solution["gRecaptchaResponse"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing gRecaptchaResponse in solution".into())
    }

    async fn solve_recaptcha_v3(
        &self,
        sitekey: &str,
        page_url: &str,
        action: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let task = serde_json::json!({
            "type": "RecaptchaV3TaskProxyless",
            "websiteURL": page_url,
            "websiteKey": sitekey,
            "pageAction": action,
            "minScore": 0.3,
        });
        let solution = self.create_and_poll(task).await?;
        solution["gRecaptchaResponse"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing gRecaptchaResponse in solution".into())
    }

    async fn solve_hcaptcha(
        &self,
        sitekey: &str,
        page_url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let task = serde_json::json!({
            "type": "HCaptchaTaskProxyless",
            "websiteURL": page_url,
            "websiteKey": sitekey,
        });
        let solution = self.create_and_poll(task).await?;
        solution["gRecaptchaResponse"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing gRecaptchaResponse in solution".into())
    }

    async fn solve_turnstile(
        &self,
        sitekey: &str,
        page_url: &str,
        action: Option<&str>,
        data: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut task = serde_json::json!({
            "type": "TurnstileTaskProxyless",
            "websiteURL": page_url,
            "websiteKey": sitekey,
        });
        if let Some(action) = action {
            task["action"] = serde_json::Value::String(action.to_string());
        }
        if let Some(data) = data {
            task["data"] = serde_json::Value::String(data.to_string());
        }
        let solution = self.create_and_poll(task).await?;
        solution["token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing token in Turnstile solution".into())
    }

    /// Submit a task to 2Captcha API v2 and poll for the result.
    async fn create_and_poll(
        &self,
        task: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let req = CreateTaskRequest {
            client_key: self.api_key.clone(),
            task,
        };

        let resp: CreateTaskResponse = self
            .client
            .post("https://api.2captcha.com/createTask")
            .json(&req)
            .send()
            .await?
            .json()
            .await?;

        if resp.error_id != 0 {
            return Err(format!(
                "2Captcha createTask failed: {} - {}",
                resp.error_code.unwrap_or_default(),
                resp.error_description.unwrap_or_default()
            )
            .into());
        }

        let task_id = resp
            .task_id
            .ok_or("2Captcha returned no taskId")?;

        tracing::info!("Captcha task submitted, id: {}", task_id);

        // Poll for result — 2Captcha recommends 5s intervals after initial 10-20s wait
        tokio::time::sleep(Duration::from_secs(15)).await;

        for _ in 0..40 {
            let poll_req = GetTaskResultRequest {
                client_key: self.api_key.clone(),
                task_id,
            };

            let result: GetTaskResultResponse = self
                .client
                .post("https://api.2captcha.com/getTaskResult")
                .json(&poll_req)
                .send()
                .await?
                .json()
                .await?;

            if result.error_id != 0 {
                return Err(format!(
                    "2Captcha error: {}",
                    result.error_code.unwrap_or_default()
                )
                .into());
            }

            if result.status.as_deref() == Some("ready") {
                if let Some(solution) = result.solution {
                    tracing::info!("Captcha solved successfully");
                    return Ok(solution);
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        Err("Captcha solving timed out after 200s".into())
    }

    // --- Token injection ---

    async fn inject_recaptcha_token(
        &self,
        page: &Page,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let js = format!(
            r#"(() => {{
                const textarea = document.getElementById('g-recaptcha-response');
                if (textarea) {{
                    textarea.innerHTML = '{token}';
                    textarea.value = '{token}';
                }}
                document.querySelectorAll('[name="g-recaptcha-response"]').forEach(el => {{
                    el.innerHTML = '{token}';
                    el.value = '{token}';
                }});
                if (typeof ___grecaptcha_cfg !== 'undefined') {{
                    Object.entries(___grecaptcha_cfg.clients).forEach(([key, client]) => {{
                        Object.entries(client).forEach(([_, value]) => {{
                            if (value && value.callback) {{
                                value.callback('{token}');
                            }}
                        }});
                    }});
                }}
            }})()"#,
            token = token
        );
        page.evaluate(js).await?;
        Ok(())
    }

    async fn inject_hcaptcha_token(
        &self,
        page: &Page,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let js = format!(
            r#"(() => {{
                document.querySelectorAll('[name="h-captcha-response"]').forEach(el => {{
                    el.value = '{token}';
                }});
                document.querySelectorAll('[name="g-recaptcha-response"]').forEach(el => {{
                    el.value = '{token}';
                }});
            }})()"#,
            token = token
        );
        page.evaluate(js).await?;
        Ok(())
    }

    async fn inject_turnstile_token(
        &self,
        page: &Page,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let js = format!(
            r#"(() => {{
                // Inject into all Turnstile response fields
                document.querySelectorAll('[name="cf-turnstile-response"]').forEach(el => {{
                    el.value = '{token}';
                }});
                // Trigger Turnstile callback if available
                const widgets = document.querySelectorAll('.cf-turnstile');
                widgets.forEach(w => {{
                    const cb = w.getAttribute('data-callback');
                    if (cb && typeof window[cb] === 'function') {{
                        window[cb]('{token}');
                    }}
                }});
                // For managed challenge pages, submit the form
                const form = document.querySelector('form[action*="challenge"]');
                if (form) {{
                    const input = form.querySelector('[name="cf-turnstile-response"]');
                    if (input) input.value = '{token}';
                    form.submit();
                }}
            }})()"#,
            token = token
        );
        page.evaluate(js).await?;

        // Wait for challenge page redirect
        tokio::time::sleep(Duration::from_millis(3000)).await;

        Ok(())
    }
}
