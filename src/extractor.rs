use regex::Regex;
use scraper::{Html, Selector};

use crate::browser::Link;

/// Default max output length in chars (~50k tokens at 4 chars/token).
const DEFAULT_MAX_CHARS: usize = 200_000;

/// Convert rendered HTML to clean markdown, stripping noise.
/// `max_chars` truncates the output to limit token cost.
pub fn html_to_markdown(html: &str, max_chars: Option<usize>) -> String {
    let cleaned = strip_non_content(html);
    let cleaned = strip_inline_styles_and_css(&cleaned);
    let md = html2md::parse_html(&cleaned);
    let md = clean_markdown(&md);

    // If output is mostly table pipes (layout tables, not data tables),
    // fall back to plain text extraction
    let md = if is_table_noise(&md) {
        let text = extract_plain_text(html);
        clean_markdown(&text)
    } else {
        md
    };

    truncate_cleanly(&md, max_chars.unwrap_or(DEFAULT_MAX_CHARS))
}

/// Extract all links from the page.
pub fn extract_links(html: &str) -> Vec<Link> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").unwrap();

    document
        .select(&selector)
        .filter_map(|el| {
            let href = el.value().attr("href")?.to_string();
            if href.starts_with("javascript:") || href.starts_with('#') || href.is_empty() {
                return None;
            }
            let text = el.text().collect::<String>().trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(Link { text, href })
        })
        .collect()
}

/// Try to extract main content via semantic selectors.
/// Uses a scoring heuristic: pick the container with the most text content.
fn strip_non_content(html: &str) -> String {
    let document = Html::parse_document(html);

    // Phase 1: Try semantic content containers, pick the one with most text
    let content_selectors = [
        "main",
        "article",
        "[role=\"main\"]",
        "#content",
        "#main-content",
        ".main-content",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".page-content",
        "#bodyContent",       // Wikipedia
        "#mw-content-text",   // Wikipedia
        ".repository-content", // GitHub
        ".pagehead",          // GitHub (combined with repo content)
    ];

    let mut best_html = String::new();
    let mut best_text_len = 0;

    for sel_str in &content_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in document.select(&sel) {
                let text_len: usize = el.text().map(|t| t.trim().len()).sum();
                if text_len > best_text_len {
                    best_text_len = text_len;
                    best_html = el.html();
                }
            }
        }
    }

    // If we found a good content container (>200 chars of text), use it
    if best_text_len > 200 {
        return remove_noise_elements(&best_html);
    }

    // Phase 2: Fall back to <body> with aggressive noise removal
    let body_sel = Selector::parse("body").unwrap();
    let body_html = match document.select(&body_sel).next() {
        Some(el) => el.html(),
        None => html.to_string(),
    };

    remove_noise_elements(&body_html)
}

/// Remove noisy elements from an HTML fragment.
fn remove_noise_elements(html: &str) -> String {
    let doc = Html::parse_fragment(html);

    let noise_selectors: Vec<Selector> = [
        "script",
        "style",
        "noscript",
        "nav",
        "footer",
        "header",
        "iframe",
        "svg",
        "img",     // images can't be read by agents
        "video",
        "audio",
        "canvas",
        "details", // collapsed details add noise
        "aside",
        ".sidebar",
        ".side-bar",
        "#sidebar",
        ".toc",
        "#toc",
        ".table-of-contents",
        ".mw-jump-link",              // Wikipedia skip links
        ".navbox",                     // Wikipedia navboxes
        ".catlinks",                   // Wikipedia categories
        ".mw-editsection",            // Wikipedia edit links
        ".reflist",                    // Wikipedia reference lists
        ".reference",                  // Wikipedia inline refs
        ".infobox",                    // Wikipedia infoboxes (often huge)
        ".mw-indicators",             // Wikipedia indicators
        "#mw-navigation",             // Wikipedia nav
        "#mw-panel",                  // Wikipedia sidebar
        "#mw-head",                   // Wikipedia header
        "#p-lang-btn",                // Wikipedia language button
        ".interlanguage-link",        // Wikipedia interlanguage links
        "#p-lang",                    // Wikipedia language list
        ".mw-portlet-lang",           // Wikipedia language portlet
        ".vector-header-container",   // Wikipedia header
        ".vector-column-start",       // Wikipedia sidebar
        ".vector-column-end",         // Wikipedia sidebar (right)
        ".vector-menu",               // Wikipedia menus
        ".vector-toc",                // Wikipedia table of contents
        "#vector-toc",                // Wikipedia TOC
        ".mw-table-of-contents-container", // Wikipedia TOC container
        "#toc",                       // Generic TOC
        ".toc",                       // Generic TOC
        ".sidebar",                   // Generic sidebars
        "#mw-panel-toc",              // Wikipedia panel TOC
        "[role=\"navigation\"]",
        "[role=\"banner\"]",
        "[role=\"contentinfo\"]",
        "[role=\"complementary\"]",
        "[aria-hidden=\"true\"]",
        "[data-testid=\"nav\"]",
        "[data-testid=\"footer\"]",
        ".cookie-banner",
        ".consent-banner",
        "#cookie-consent",
        ".ad",
        ".ads",
        ".advertisement",
        ".social-share",
        ".share-buttons",
        ".related-posts",
        ".comments",
        "#disqus_thread",
        "select",                      // dropdowns
        ".dropdown-menu",
        ".select-menu",
    ]
    .iter()
    .filter_map(|s| Selector::parse(s).ok())
    .collect();

    // Collect noise fragments to remove
    let mut noise_fragments: Vec<String> = Vec::new();
    for sel in &noise_selectors {
        for el in doc.select(sel) {
            noise_fragments.push(el.html());
        }
    }

    // Sort by length descending so we remove larger fragments first
    // (avoids partial matches from nested elements)
    noise_fragments.sort_by(|a, b| b.len().cmp(&a.len()));

    let mut cleaned = html.to_string();
    for fragment in &noise_fragments {
        if let Some(pos) = cleaned.find(fragment.as_str()) {
            cleaned.replace_range(pos..pos + fragment.len(), "");
        }
    }

    cleaned
}

/// Strip inline styles, <style> blocks, CSS class definitions, and data attributes
/// that leak into markdown output.
fn strip_inline_styles_and_css(html: &str) -> String {
    let mut result = html.to_string();

    // Remove <style> tags and contents (in case any survived)
    let style_re = Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap();
    result = style_re.replace_all(&result, "").to_string();

    // Remove style attributes
    let style_attr_re = Regex::new(r#"\s*style\s*=\s*"[^"]*""#).unwrap();
    result = style_attr_re.replace_all(&result, "").to_string();
    let style_attr_sq_re = Regex::new(r#"\s*style\s*=\s*'[^']*'"#).unwrap();
    result = style_attr_sq_re.replace_all(&result, "").to_string();

    // Remove class attributes
    let class_re = Regex::new(r#"\s*class\s*=\s*"[^"]*""#).unwrap();
    result = class_re.replace_all(&result, "").to_string();

    // Remove data-* attributes
    let data_re = Regex::new(r#"\s*data-[\w-]+\s*=\s*"[^"]*""#).unwrap();
    result = data_re.replace_all(&result, "").to_string();

    // Remove id attributes (they add noise to markdown)
    let id_re = Regex::new(r#"\s*id\s*=\s*"[^"]*""#).unwrap();
    result = id_re.replace_all(&result, "").to_string();

    result
}

/// Clean up markdown output: remove CSS artifacts, excessive formatting, empty links.
fn clean_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());

    for line in md.lines() {
        let trimmed = line.trim();

        // Skip lines that are CSS rules or class definitions
        if is_css_line(trimmed) {
            continue;
        }

        // Skip empty markdown links like [](...)
        if trimmed == "[]" || trimmed.starts_with("[](") {
            continue;
        }

        // Skip lines that are just whitespace or special chars
        if trimmed.is_empty()
            || trimmed == "|"
            || trimmed == "---"
            || trimmed == "***"
            || trimmed == "* * *"
        {
            result.push('\n');
            continue;
        }

        // Skip lines that are purely image references (agents can't see them)
        if trimmed.starts_with("![") && !trimmed.contains(|c: char| c.is_alphabetic()) {
            continue;
        }

        // Skip anchor-only lines like ](#some_section)
        if trimmed.starts_with("](#") || trimmed == "](#)" {
            continue;
        }

        // Skip lines that are just a link to an external Wikipedia language version
        if is_interlanguage_link(trimmed) {
            continue;
        }

        // Skip "toggle ... subsection" and "move to sidebar" navigation artifacts
        if trimmed.starts_with("Toggle ") && trimmed.ends_with(" subsection") {
            continue;
        }
        if trimmed == "move to sidebar hide" || trimmed == "Toggle the table of contents" {
            continue;
        }

        // Skip table-only formatting lines (pipes with no content)
        if trimmed.chars().all(|c| c == '|' || c == '-' || c == ' ' || c == ':') && trimmed.contains('|') {
            continue;
        }

        result.push_str(trimmed);
        result.push('\n');
    }

    // Collapse excessive whitespace
    collapse_whitespace(&result)
}

/// Detect interlanguage links (e.g., Wikipedia language links).
fn is_interlanguage_link(line: &str) -> bool {
    // Pattern: "* [LangName](https://xx.wikipedia.org/...)"
    if line.starts_with("* [") && line.contains(".wikipedia.org/") {
        return true;
    }
    // Bare language links
    if line.starts_with('[') && line.contains(".wikipedia.org/") && line.ends_with(')') {
        return true;
    }
    false
}

/// Detect lines that look like CSS rather than content.
fn is_css_line(line: &str) -> bool {
    // CSS property patterns: "property: value;"
    let css_prop = Regex::new(r"^[\w-]+\s*:\s*[^;]+;\s*$").unwrap();
    // CSS selector patterns: ".class {" or "#id {"
    let css_selector = Regex::new(r"^[.#\[\w][\w\s,.#\[\]=>:~*+-]*\{\s*$").unwrap();
    // Just a closing brace
    let closing_brace = line == "}";
    // CSS @rules
    let at_rule = line.starts_with('@');

    // Lines that are clearly CSS
    if css_prop.is_match(line) || css_selector.is_match(line) || closing_brace || at_rule {
        return true;
    }

    // Common CSS patterns that leak through
    let css_keywords = [
        "display:",
        "position:",
        "margin:",
        "padding:",
        "background:",
        "font-size:",
        "color:",
        "border:",
        "width:",
        "height:",
        "overflow:",
        "z-index:",
        "max-height:",
        "max-width:",
        "min-height:",
        "min-width:",
        "line-height:",
        "text-align:",
        "vertical-align:",
        "flex:",
        "grid:",
        "opacity:",
        "transform:",
        "transition:",
        "animation:",
        "visibility:",
        "cursor:",
        "box-shadow:",
        "var(--",
    ];

    for kw in &css_keywords {
        if line.contains(kw) && !line.contains("```") {
            return true;
        }
    }

    false
}

/// Detect if markdown output is mostly table formatting or whitespace with little real content.
fn is_table_noise(md: &str) -> bool {
    let total_chars = md.len();
    if total_chars < 100 {
        return false;
    }

    let total_lines = md.lines().count().max(1);

    // Count lines that have actual readable content (not just formatting)
    let substantive_lines = md
        .lines()
        .filter(|l| {
            let t = l.trim();
            let text_only: String = t.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
            text_only.trim().len() > 20
                && !t.starts_with('|')
                && !t.chars().all(|c| c == '|' || c == '-' || c == ' ' || c == ':')
        })
        .count();

    // Count whitespace-dominated lines (>80% whitespace)
    let whitespace_lines = md
        .lines()
        .filter(|l| {
            let total = l.len().max(1);
            let ws = l.chars().filter(|c| c.is_whitespace() || *c == '|' || *c == '-').count();
            total > 10 && (ws as f64 / total as f64) > 0.8
        })
        .count();

    // If >70% of lines are whitespace/pipe-dominated and <5% are substantive
    let whitespace_ratio = whitespace_lines as f64 / total_lines as f64;
    let substantive_ratio = substantive_lines as f64 / total_lines as f64;

    whitespace_ratio > 0.7 || substantive_ratio < 0.05
}

/// Extract plain text from HTML, ignoring all formatting.
/// Used as fallback when markdown conversion produces mostly table noise.
fn extract_plain_text(html: &str) -> String {
    // Remove noise elements first
    let cleaned = strip_non_content(html);
    let doc = Html::parse_fragment(&cleaned);

    // Get all text nodes
    let mut texts: Vec<String> = Vec::new();
    let root = Selector::parse("body, html, *").unwrap();
    for el in doc.select(&root) {
        for text in el.text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                texts.push(trimmed.to_string());
            }
        }
    }

    // Deduplicate consecutive identical lines
    let mut result = Vec::new();
    let mut prev = String::new();
    for t in texts {
        if t != prev {
            result.push(t.clone());
            prev = t;
        }
    }

    result.join("\n")
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

/// Truncate markdown to a max length, cutting at a paragraph boundary.
fn truncate_cleanly(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    // Find the last double-newline before the limit
    let search_region = &text[..max_chars];
    if let Some(pos) = search_region.rfind("\n\n") {
        let mut truncated = text[..pos].to_string();
        truncated.push_str("\n\n[...truncated]");
        return truncated;
    }

    // Fall back to last single newline
    if let Some(pos) = search_region.rfind('\n') {
        let mut truncated = text[..pos].to_string();
        truncated.push_str("\n\n[...truncated]");
        return truncated;
    }

    // Hard cut
    let mut truncated = text[..max_chars].to_string();
    truncated.push_str("\n\n[...truncated]");
    truncated
}
