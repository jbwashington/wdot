use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub language: Option<String>,
    pub opengraph: OpenGraph,
    pub twitter_card: TwitterCard,
    pub meta_tags: Vec<MetaTag>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OpenGraph {
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub og_url: Option<String>,
    pub og_type: Option<String>,
    pub og_site_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TwitterCard {
    pub card: Option<String>,
    pub site: Option<String>,
    pub creator: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetaTag {
    pub name: Option<String>,
    pub property: Option<String>,
    pub content: String,
}

/// Extract all metadata from rendered HTML.
pub fn extract(html: &str) -> PageMetadata {
    let document = Html::parse_document(html);

    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string());

    let description = get_meta_content(&document, "description");
    let canonical_url = Selector::parse("link[rel='canonical']")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("href").map(|s| s.to_string()));

    let language = Selector::parse("html[lang]")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("lang").map(|s| s.to_string()));

    let opengraph = OpenGraph {
        og_title: get_og(&document, "og:title"),
        og_description: get_og(&document, "og:description"),
        og_image: get_og(&document, "og:image"),
        og_url: get_og(&document, "og:url"),
        og_type: get_og(&document, "og:type"),
        og_site_name: get_og(&document, "og:site_name"),
    };

    let twitter_card = TwitterCard {
        card: get_meta_content(&document, "twitter:card"),
        site: get_meta_content(&document, "twitter:site"),
        creator: get_meta_content(&document, "twitter:creator"),
        title: get_meta_content(&document, "twitter:title"),
        description: get_meta_content(&document, "twitter:description"),
        image: get_meta_content(&document, "twitter:image"),
    };

    let meta_tags = extract_all_meta(&document);

    PageMetadata {
        title,
        description,
        canonical_url,
        language,
        opengraph,
        twitter_card,
        meta_tags,
    }
}

fn get_meta_content(document: &Html, name: &str) -> Option<String> {
    let sel_str = format!("meta[name='{}']", name);
    Selector::parse(&sel_str)
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
}

fn get_og(document: &Html, property: &str) -> Option<String> {
    let sel_str = format!("meta[property='{}']", property);
    Selector::parse(&sel_str)
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
}

fn extract_all_meta(document: &Html) -> Vec<MetaTag> {
    let mut tags = Vec::new();
    if let Ok(sel) = Selector::parse("meta[content]") {
        for el in document.select(&sel) {
            let content = el.value().attr("content").unwrap_or("").to_string();
            if content.is_empty() {
                continue;
            }
            tags.push(MetaTag {
                name: el.value().attr("name").map(|s| s.to_string()),
                property: el.value().attr("property").map(|s| s.to_string()),
                content,
            });
        }
    }
    tags
}
