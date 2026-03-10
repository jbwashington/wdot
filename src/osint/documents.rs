use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DocumentLink {
    pub url: String,
    pub filename: String,
    pub extension: String,
}

const DOC_EXTENSIONS: &[&str] = &[
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".csv", ".ppt", ".pptx",
    ".txt", ".rtf", ".odt", ".ods", ".zip", ".rar", ".7z", ".tar",
    ".gz", ".json", ".xml", ".yaml", ".yml",
];

/// Discover downloadable documents linked from the page.
pub fn extract(html: &str, base_url: &str) -> Vec<DocumentLink> {
    let document = Html::parse_document(html);
    let mut docs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let lower = href.to_lowercase();
                for ext in DOC_EXTENSIONS {
                    if lower.ends_with(ext) || lower.contains(&format!("{}?", ext)) {
                        let full_url = resolve_url(base_url, href);
                        if !seen.contains(&full_url) {
                            seen.insert(full_url.clone());
                            let filename = href
                                .rsplit('/')
                                .next()
                                .unwrap_or(href)
                                .split('?')
                                .next()
                                .unwrap_or("")
                                .to_string();
                            docs.push(DocumentLink {
                                url: full_url,
                                filename,
                                extension: ext.trim_start_matches('.').to_string(),
                            });
                        }
                        break;
                    }
                }
            }
        }
    }

    docs
}

fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        return format!("https:{}", href);
    }
    if href.starts_with('/') {
        // Extract origin from base
        if let Some(idx) = base.find("://") {
            if let Some(slash) = base[idx + 3..].find('/') {
                return format!("{}{}", &base[..idx + 3 + slash], href);
            }
        }
        return format!("{}{}", base.trim_end_matches('/'), href);
    }
    format!("{}/{}", base.trim_end_matches('/'), href)
}
