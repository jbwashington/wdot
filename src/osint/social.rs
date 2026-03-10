use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SocialProfile {
    pub platform: String,
    pub url: String,
    pub username: Option<String>,
}

/// Known social platform URL patterns.
const PLATFORMS: &[(&str, &str, &str)] = &[
    ("twitter.com/", "Twitter/X", r"twitter\.com/([^/?#]+)"),
    ("x.com/", "Twitter/X", r"x\.com/([^/?#]+)"),
    ("linkedin.com/in/", "LinkedIn", r"linkedin\.com/in/([^/?#]+)"),
    ("linkedin.com/company/", "LinkedIn", r"linkedin\.com/company/([^/?#]+)"),
    ("github.com/", "GitHub", r"github\.com/([^/?#]+)"),
    ("facebook.com/", "Facebook", r"facebook\.com/([^/?#]+)"),
    ("instagram.com/", "Instagram", r"instagram\.com/([^/?#]+)"),
    ("youtube.com/", "YouTube", r"youtube\.com/(?:@|channel/|user/)([^/?#]+)"),
    ("tiktok.com/@", "TikTok", r"tiktok\.com/@([^/?#]+)"),
    ("reddit.com/r/", "Reddit", r"reddit\.com/r/([^/?#]+)"),
    ("reddit.com/u/", "Reddit", r"reddit\.com/u/([^/?#]+)"),
    ("mastodon", "Mastodon", r"@([^@]+)@([^/?#\s]+)"),
    ("discord.gg/", "Discord", r"discord\.gg/([^/?#]+)"),
    ("t.me/", "Telegram", r"t\.me/([^/?#]+)"),
    ("medium.com/@", "Medium", r"medium\.com/@([^/?#]+)"),
    ("dev.to/", "DEV Community", r"dev\.to/([^/?#]+)"),
];

/// Discover social media profiles from page links and metadata.
pub fn extract(html: &str) -> Vec<SocialProfile> {
    let document = Html::parse_document(html);
    let mut profiles = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    // Check all links
    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                for (pattern, platform, username_re) in PLATFORMS {
                    if href.contains(pattern) && !seen_urls.contains(href) {
                        seen_urls.insert(href.to_string());
                        let username = regex::Regex::new(username_re)
                            .ok()
                            .and_then(|re| re.captures(href))
                            .and_then(|cap| cap.get(1))
                            .map(|m| m.as_str().to_string());

                        profiles.push(SocialProfile {
                            platform: platform.to_string(),
                            url: href.to_string(),
                            username,
                        });
                    }
                }
            }
        }
    }

    // Check <link rel="me"> (IndieWeb standard)
    if let Ok(sel) = Selector::parse("link[rel='me']") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                if !seen_urls.contains(href) {
                    for (pattern, platform, _) in PLATFORMS {
                        if href.contains(pattern) {
                            seen_urls.insert(href.to_string());
                            profiles.push(SocialProfile {
                                platform: platform.to_string(),
                                url: href.to_string(),
                                username: None,
                            });
                        }
                    }
                }
            }
        }
    }

    profiles
}
