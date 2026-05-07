use std::path::Path;
use uaparser::{Parser, UserAgentParser};

pub struct UaParser {
    parser: Option<UserAgentParser>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedUa {
    pub platform: Option<String>,
    pub browser: Option<String>,
    pub device: Option<String>, // Mobile | Tablet | Desktop
    pub is_bot: bool,
    pub bot_name: Option<String>,
}

impl UaParser {
    /// Loads regexes.yaml from `data_dir/regexes.yaml` if present, else falls
    /// back to a substring heuristic. Use `ensure_regexes` to download.
    pub fn load(path: &std::path::Path) -> Self {
        if path.exists() {
            if let Some(parser) = try_load(path) {
                tracing::info!("uaparser regexes loaded from {}", path.display());
                return Self { parser: Some(parser) };
            }
        }
        tracing::warn!(
            "uaparser regexes.yaml not found — ua parsing falls back to a substring heuristic until refresh"
        );
        Self { parser: None }
    }

    pub fn reload(&mut self, path: &std::path::Path) {
        if path.exists() {
            if let Some(parser) = try_load(path) {
                self.parser = Some(parser);
            }
        }
    }

    pub fn parse(&self, ua: &str) -> ParsedUa {
        if let Some(parser) = &self.parser {
            let client = parser.parse(ua);
            let platform = match client.os.family.as_ref() {
                "Other" => None,
                other => Some(other.to_string()),
            };
            let browser = match client.user_agent.family.as_ref() {
                "Other" => None,
                other => Some(other.to_string()),
            };
            let device_family = client.device.family.as_ref();
            let is_bot = matches!(
                device_family,
                "Spider" | "Spider Desktop" | "Spider Smartphone" | "Spider Tablet"
            ) || classify_bot_by_ua(ua);

            let device = if is_bot {
                None
            } else {
                Some(classify_device(ua, device_family).to_string())
            };
            let bot_name = if is_bot {
                Some(client.user_agent.family.to_string()).filter(|s| s != "Other")
            } else {
                None
            };
            return ParsedUa { platform, browser, device, is_bot, bot_name };
        }
        // Fallback heuristic so the collector still works without regexes.yaml.
        let is_bot = classify_bot_by_ua(ua);
        let device = if is_bot { None } else { Some(classify_device(ua, "").to_string()) };
        ParsedUa {
            platform: None,
            browser: None,
            device,
            is_bot,
            bot_name: if is_bot { Some("Unknown bot".to_string()) } else { None },
        }
    }
}

fn try_load(path: &Path) -> Option<UserAgentParser> {
    UserAgentParser::builder()
        .with_unicode_support(false)
        .build_from_yaml(path.to_string_lossy().as_ref())
        .ok()
}

/// Download the canonical ua-parser regexes.yaml on first boot if missing.
/// Source: https://github.com/ua-parser/uap-core (Apache-2.0).
pub async fn ensure_regexes(dest: &Path) -> anyhow::Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
    let url = "https://raw.githubusercontent.com/ua-parser/uap-core/master/regexes.yaml";
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &bytes)?;
    tracing::info!("downloaded ua-parser regexes to {}", dest.display());
    Ok(true)
}

fn classify_bot_by_ua(ua: &str) -> bool {
    let ua = ua.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "bot", "crawl", "spider", "slurp", "facebookexternalhit", "ahrefs", "semrush",
        "petalbot", "yandex", "bingpreview", "duckduckgo", "discordbot", "whatsapp",
        "telegrambot", "applebot", "linkedinbot", "embedly", "headlesschrome",
        "phantomjs", "lighthouse", "pingdom", "uptimerobot", "monitor",
    ];
    NEEDLES.iter().any(|n| ua.contains(n))
}

fn classify_device(ua: &str, family: &str) -> &'static str {
    let lower = ua.to_ascii_lowercase();
    let tablet = matches!(family, "iPad" | "Tablet")
        || lower.contains("tablet")
        || lower.contains("ipad");
    if tablet {
        return "Tablet";
    }
    let mobile = matches!(family, "iPhone" | "iPod" | "Generic Smartphone")
        || lower.contains("mobile")
        || lower.contains("iphone")
        || lower.contains("android");
    if mobile {
        "Mobile"
    } else {
        "Desktop"
    }
}
