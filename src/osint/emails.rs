use regex::Regex;
use scraper::{Html, Selector};

/// Extract email addresses from rendered HTML.
pub fn extract(html: &str) -> Vec<String> {
    let mut emails = std::collections::HashSet::new();

    // Regex scan of all text content
    let email_re = Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    for cap in email_re.find_iter(html) {
        let email = cap.as_str().to_lowercase();
        // Filter out obvious false positives
        if !email.ends_with(".png")
            && !email.ends_with(".jpg")
            && !email.ends_with(".svg")
            && !email.contains("example.com")
            && !email.contains("sentry.io")
            && !email.contains("w3.org")
        {
            emails.insert(email);
        }
    }

    // Also check mailto: links
    let document = Html::parse_document(html);
    if let Ok(sel) = Selector::parse("a[href^='mailto:']") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let addr = href.trim_start_matches("mailto:").split('?').next().unwrap_or("");
                if !addr.is_empty() {
                    emails.insert(addr.to_lowercase());
                }
            }
        }
    }

    let mut result: Vec<String> = emails.into_iter().collect();
    result.sort();
    result
}
