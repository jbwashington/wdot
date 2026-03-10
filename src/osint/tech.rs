use chromiumoxide::page::Page;
use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Technology {
    pub name: String,
    pub version: Option<String>,
    pub category: TechCategory,
}

#[derive(Debug, Clone, Serialize)]
pub enum TechCategory {
    Framework,
    CMS,
    Analytics,
    CDN,
    Server,
    Library,
    BuildTool,
    Other,
}

/// Fingerprint technologies from HTML content (static analysis).
pub fn extract_from_html(html: &str) -> Vec<Technology> {
    let document = Html::parse_document(html);
    let mut techs = Vec::new();

    // Meta generator tag
    if let Ok(sel) = Selector::parse("meta[name='generator']") {
        for el in document.select(&sel) {
            if let Some(content) = el.value().attr("content") {
                techs.push(Technology {
                    name: content.to_string(),
                    version: None,
                    category: TechCategory::CMS,
                });
            }
        }
    }

    // Script src patterns
    if let Ok(sel) = Selector::parse("script[src]") {
        for el in document.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                detect_script_tech(src, &mut techs);
            }
        }
    }

    // Link href patterns (CSS frameworks, etc.)
    if let Ok(sel) = Selector::parse("link[href][rel='stylesheet']") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                detect_css_tech(href, &mut techs);
            }
        }
    }

    techs
}

/// Fingerprint technologies via JavaScript runtime detection.
pub async fn extract_from_js(page: &Page) -> Vec<Technology> {
    let js = r#"(() => {
        const techs = [];
        if (window.jQuery) techs.push({name: "jQuery", version: jQuery.fn?.jquery || null, cat: "Library"});
        if (window.React || document.querySelector('[data-reactroot]')) techs.push({name: "React", version: window.React?.version || null, cat: "Framework"});
        if (window.__NEXT_DATA__) techs.push({name: "Next.js", version: window.__NEXT_DATA__?.nextExport ? null : null, cat: "Framework"});
        if (window.Vue) techs.push({name: "Vue.js", version: window.Vue?.version || null, cat: "Framework"});
        if (window.__NUXT__) techs.push({name: "Nuxt.js", version: null, cat: "Framework"});
        if (window.angular) techs.push({name: "AngularJS", version: window.angular?.version?.full || null, cat: "Framework"});
        if (window.ng) techs.push({name: "Angular", version: null, cat: "Framework"});
        if (window.Svelte || document.querySelector('[class*="svelte-"]')) techs.push({name: "Svelte", version: null, cat: "Framework"});
        if (window.Shopify) techs.push({name: "Shopify", version: null, cat: "CMS"});
        if (window.wp) techs.push({name: "WordPress", version: null, cat: "CMS"});
        if (window.Drupal) techs.push({name: "Drupal", version: null, cat: "CMS"});
        if (window.ga || window.gtag) techs.push({name: "Google Analytics", version: null, cat: "Analytics"});
        if (window._satellite) techs.push({name: "Adobe Analytics", version: null, cat: "Analytics"});
        if (window.mixpanel) techs.push({name: "Mixpanel", version: null, cat: "Analytics"});
        if (window.amplitude) techs.push({name: "Amplitude", version: null, cat: "Analytics"});
        if (document.querySelector('script[src*="segment.com"]')) techs.push({name: "Segment", version: null, cat: "Analytics"});
        if (window.Ember) techs.push({name: "Ember.js", version: window.Ember?.VERSION || null, cat: "Framework"});
        if (window.Backbone) techs.push({name: "Backbone.js", version: window.Backbone?.VERSION || null, cat: "Framework"});
        return JSON.stringify(techs);
    })()"#;

    let result = page.evaluate(js).await;
    match result {
        Ok(val) => {
            let json_str = val.into_value::<String>().unwrap_or_default();
            serde_json::from_str::<Vec<TechDetection>>(&json_str)
                .unwrap_or_default()
                .into_iter()
                .map(|t| Technology {
                    name: t.name,
                    version: t.version,
                    category: match t.cat.as_str() {
                        "Framework" => TechCategory::Framework,
                        "CMS" => TechCategory::CMS,
                        "Analytics" => TechCategory::Analytics,
                        "Library" => TechCategory::Library,
                        _ => TechCategory::Other,
                    },
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

#[derive(serde::Deserialize)]
struct TechDetection {
    name: String,
    version: Option<String>,
    cat: String,
}

fn detect_script_tech(src: &str, techs: &mut Vec<Technology>) {
    let patterns: &[(&str, &str, TechCategory)] = &[
        ("wp-content", "WordPress", TechCategory::CMS),
        ("wp-includes", "WordPress", TechCategory::CMS),
        ("cdn.shopify.com", "Shopify", TechCategory::CMS),
        ("googletagmanager.com", "Google Tag Manager", TechCategory::Analytics),
        ("google-analytics.com", "Google Analytics", TechCategory::Analytics),
        ("cdn.jsdelivr.net", "jsDelivr CDN", TechCategory::CDN),
        ("cdnjs.cloudflare.com", "Cloudflare CDN", TechCategory::CDN),
        ("unpkg.com", "unpkg CDN", TechCategory::CDN),
        ("ajax.googleapis.com", "Google CDN", TechCategory::CDN),
        ("cloudfront.net", "AWS CloudFront", TechCategory::CDN),
        ("akamai", "Akamai CDN", TechCategory::CDN),
        ("bootstrap", "Bootstrap", TechCategory::Library),
        ("tailwind", "Tailwind CSS", TechCategory::Library),
        ("react", "React", TechCategory::Framework),
        ("vue", "Vue.js", TechCategory::Framework),
        ("angular", "Angular", TechCategory::Framework),
        ("stripe.com/v3", "Stripe", TechCategory::Library),
    ];

    for (pattern, name, cat) in patterns {
        if src.contains(pattern) {
            techs.push(Technology {
                name: name.to_string(),
                version: None,
                category: cat.clone(),
            });
        }
    }
}

fn detect_css_tech(href: &str, techs: &mut Vec<Technology>) {
    if href.contains("bootstrap") {
        techs.push(Technology { name: "Bootstrap".into(), version: None, category: TechCategory::Library });
    }
    if href.contains("tailwind") {
        techs.push(Technology { name: "Tailwind CSS".into(), version: None, category: TechCategory::Library });
    }
    if href.contains("font-awesome") || href.contains("fontawesome") {
        techs.push(Technology { name: "Font Awesome".into(), version: None, category: TechCategory::Library });
    }
}
