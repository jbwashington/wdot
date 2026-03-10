pub mod dns;
pub mod documents;
pub mod emails;
pub mod metadata;
pub mod social;
pub mod tech;

use serde::Serialize;
use std::sync::Arc;

use crate::browser::BrowserPool;

/// Orchestrates OSINT data collection across all sub-modules.
pub struct OsintEngine {
    browser: Arc<BrowserPool>,
}

#[derive(Debug, Serialize)]
pub struct OsintReport {
    pub target: String,
    pub emails: Vec<String>,
    pub social_profiles: Vec<social::SocialProfile>,
    pub dns_info: Option<dns::DnsInfo>,
    pub metadata: metadata::PageMetadata,
    pub documents: Vec<documents::DocumentLink>,
    pub technologies: Vec<tech::Technology>,
    pub collected_at: String,
}

impl OsintEngine {
    pub fn new(browser: Arc<BrowserPool>) -> Self {
        Self { browser }
    }

    /// Run a full OSINT scan on a target URL.
    pub async fn scan(
        &self,
        url: &str,
    ) -> Result<OsintReport, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch the page
        let result = self.browser.fetch(url, None, Some(30_000)).await?;
        let html = &result.raw_html;

        // Extract domain for DNS lookup
        let domain = extract_domain(url);

        // Run all extractors
        let email_list = emails::extract(html);
        let social_list = social::extract(html);
        let meta = metadata::extract(html);
        let doc_list = documents::extract(html, url);
        let tech_list = tech::extract_from_html(html);

        // DNS lookup (don't fail the whole scan if this errors)
        let dns_info = if let Some(ref d) = domain {
            dns::lookup(d).await.ok()
        } else {
            None
        };

        // JS-based tech detection would require holding the page open,
        // which we don't do in the current architecture. The HTML-based
        // detection covers the most common cases.

        let now = chrono::Utc::now().to_rfc3339();

        Ok(OsintReport {
            target: url.to_string(),
            emails: email_list,
            social_profiles: social_list,
            dns_info,
            metadata: meta,
            documents: doc_list,
            technologies: tech_list,
            collected_at: now,
        })
    }
}

fn extract_domain(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme.split('/').next()?;
    let domain = host.split(':').next()?;
    Some(domain.to_string())
}
