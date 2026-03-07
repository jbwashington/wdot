use scraper::{Html, Selector};

use crate::browser::Link;

/// Convert rendered HTML to clean markdown, stripping nav/header/footer/script noise.
pub fn html_to_markdown(html: &str) -> String {
    let cleaned = strip_non_content(html);
    let md = html2md::parse_html(&cleaned);
    collapse_whitespace(&md)
}

/// Extract all links from the page.
pub fn extract_links(html: &str) -> Vec<Link> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").unwrap();

    document
        .select(&selector)
        .filter_map(|el| {
            let href = el.value().attr("href")?.to_string();
            if href.starts_with("javascript:") || href.is_empty() {
                return None;
            }
            let text = el.text().collect::<String>().trim().to_string();
            Some(Link { text, href })
        })
        .collect()
}

/// Try to extract main content via <main>, <article>, or [role="main"].
/// If none found, fall back to <body> with noisy elements removed.
fn strip_non_content(html: &str) -> String {
    let document = Html::parse_document(html);

    // Try to find the main content container
    let content_selectors = ["main", "article", "[role=\"main\"]", "#content", ".content"];
    for sel_str in &content_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = document.select(&sel).next() {
                return el.html();
            }
        }
    }

    // Fallback: get body and remove noisy tags via replacement
    let body_sel = Selector::parse("body").unwrap();
    let body_html = match document.select(&body_sel).next() {
        Some(el) => el.html(),
        None => html.to_string(),
    };

    // Remove noisy elements by re-parsing body and excluding them
    let body_doc = Html::parse_fragment(&body_html);
    let noise_selectors: Vec<Selector> = [
        "script",
        "style",
        "noscript",
        "nav",
        "footer",
        "iframe",
        "svg",
        "[role=\"navigation\"]",
        "[role=\"banner\"]",
        "[role=\"contentinfo\"]",
        "[aria-hidden=\"true\"]",
    ]
    .iter()
    .filter_map(|s| Selector::parse(s).ok())
    .collect();

    // Collect the HTML of noisy elements to subtract them
    let mut noise_fragments: Vec<String> = Vec::new();
    for sel in &noise_selectors {
        for el in body_doc.select(sel) {
            noise_fragments.push(el.html());
        }
    }

    let mut cleaned = body_html;
    for fragment in &noise_fragments {
        if let Some(pos) = cleaned.find(fragment.as_str()) {
            cleaned.replace_range(pos..pos + fragment.len(), "");
        }
    }

    cleaned
}

/// Collapse excessive whitespace/newlines in the markdown output.
fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut consecutive_newlines = 0u32;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}
