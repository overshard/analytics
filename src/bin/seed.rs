// Seeds a "Seed Test" property with realistic-looking fake events.
//
// Usage:
//   cargo run --bin seed                  # 500 sessions, last 30 days
//   cargo run --bin seed -- 2000 60       # 2000 sessions, last 60 days
//
// Re-runs reuse the property and wipe its existing events first so the
// dashboard URL stays stable.

#[path = "../db.rs"]
#[allow(dead_code)]
mod db;

use anyhow::Result;
use chrono::Utc;
use rand::prelude::*;
use sqlx::{SqliteConnection, SqlitePool};
use std::path::PathBuf;
use uuid::Uuid;

const PROPERTY_NAME: &str = "Seed Test";

const URLS: &[(&str, &str, u32)] = &[
    ("/", "Home", 40),
    ("/about", "About", 10),
    ("/pricing", "Pricing", 8),
    ("/docs", "Documentation", 8),
    ("/blog", "Blog", 5),
    ("/blog/getting-started", "Getting Started", 5),
    ("/blog/whats-new-in-v2", "What's New in v2", 4),
    ("/blog/case-studies", "Case Studies", 3),
    ("/contact", "Contact", 4),
    ("/login", "Log In", 4),
    ("/signup", "Sign Up", 4),
    ("/dashboard", "Dashboard", 5),
];

const REFERRERS: &[(&str, u32)] = &[
    ("", 50),
    ("google.com", 20),
    ("twitter.com", 5),
    ("news.ycombinator.com", 3),
    ("github.com", 3),
    ("reddit.com", 4),
    ("duckduckgo.com", 3),
    ("bing.com", 3),
    ("linkedin.com", 2),
    ("producthunt.com", 2),
    ("dev.to", 2),
    ("medium.com", 1),
];

struct Agent {
    ua: &'static str,
    platform: &'static str,
    browser: &'static str,
    device: &'static str,
    is_bot: bool,
    bot_name: Option<&'static str>,
    weight: u32,
}

const AGENTS: &[Agent] = &[
    Agent { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            platform: "Windows", browser: "Chrome", device: "Desktop", is_bot: false, bot_name: None, weight: 25 },
    Agent { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            platform: "Mac OS X", browser: "Chrome", device: "Desktop", is_bot: false, bot_name: None, weight: 15 },
    Agent { ua: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
            platform: "Android", browser: "Chrome Mobile", device: "Mobile", is_bot: false, bot_name: None, weight: 15 },
    Agent { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
            platform: "Mac OS X", browser: "Safari", device: "Desktop", is_bot: false, bot_name: None, weight: 10 },
    Agent { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1",
            platform: "iOS", browser: "Mobile Safari", device: "Mobile", is_bot: false, bot_name: None, weight: 15 },
    Agent { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0",
            platform: "Windows", browser: "Firefox", device: "Desktop", is_bot: false, bot_name: None, weight: 5 },
    Agent { ua: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
            platform: "Ubuntu", browser: "Firefox", device: "Desktop", is_bot: false, bot_name: None, weight: 3 },
    Agent { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
            platform: "Windows", browser: "Edge", device: "Desktop", is_bot: false, bot_name: None, weight: 8 },
    Agent { ua: "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            platform: "", browser: "", device: "", is_bot: true, bot_name: Some("Googlebot"), weight: 1 },
    Agent { ua: "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
            platform: "", browser: "", device: "", is_bot: true, bot_name: Some("bingbot"), weight: 1 },
    Agent { ua: "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
            platform: "", browser: "", device: "", is_bot: true, bot_name: Some("facebookexternalhit"), weight: 1 },
];

struct GeoRow {
    country: &'static str,
    region: &'static str,
    city: &'static str,
    lat: f64,
    lon: f64,
    weight: u32,
}

const GEO: &[GeoRow] = &[
    GeoRow { country: "US", region: "New York",         city: "New York",      lat: 40.7128, lon:  -74.0060, weight: 15 },
    GeoRow { country: "US", region: "California",       city: "Los Angeles",   lat: 34.0522, lon: -118.2437, weight: 10 },
    GeoRow { country: "US", region: "California",       city: "San Francisco", lat: 37.7749, lon: -122.4194, weight:  8 },
    GeoRow { country: "US", region: "Illinois",         city: "Chicago",       lat: 41.8781, lon:  -87.6298, weight:  5 },
    GeoRow { country: "US", region: "Texas",            city: "Austin",        lat: 30.2672, lon:  -97.7431, weight:  5 },
    GeoRow { country: "GB", region: "England",          city: "London",        lat: 51.5074, lon:   -0.1278, weight:  8 },
    GeoRow { country: "GB", region: "England",          city: "Manchester",    lat: 53.4808, lon:   -2.2426, weight:  2 },
    GeoRow { country: "DE", region: "Berlin",           city: "Berlin",        lat: 52.5200, lon:   13.4050, weight:  5 },
    GeoRow { country: "DE", region: "Bavaria",          city: "Munich",        lat: 48.1351, lon:   11.5820, weight:  3 },
    GeoRow { country: "FR", region: "Île-de-France",    city: "Paris",         lat: 48.8566, lon:    2.3522, weight:  5 },
    GeoRow { country: "CA", region: "Ontario",          city: "Toronto",       lat: 43.6532, lon:  -79.3832, weight:  4 },
    GeoRow { country: "CA", region: "British Columbia", city: "Vancouver",     lat: 49.2827, lon: -123.1207, weight:  2 },
    GeoRow { country: "AU", region: "New South Wales",  city: "Sydney",        lat: -33.8688, lon: 151.2093, weight:  3 },
    GeoRow { country: "AU", region: "Victoria",         city: "Melbourne",     lat: -37.8136, lon: 144.9631, weight:  2 },
    GeoRow { country: "JP", region: "Tokyo",            city: "Tokyo",         lat: 35.6762, lon:  139.6503, weight:  4 },
    GeoRow { country: "BR", region: "São Paulo",        city: "São Paulo",     lat: -23.5505, lon: -46.6333, weight:  3 },
    GeoRow { country: "IN", region: "Maharashtra",      city: "Mumbai",        lat: 19.0760, lon:   72.8777, weight:  3 },
    GeoRow { country: "IN", region: "Karnataka",        city: "Bangalore",     lat: 12.9716, lon:   77.5946, weight:  3 },
    GeoRow { country: "NL", region: "North Holland",    city: "Amsterdam",     lat: 52.3676, lon:    4.9041, weight:  3 },
    GeoRow { country: "ES", region: "Madrid",           city: "Madrid",        lat: 40.4168, lon:   -3.7038, weight:  2 },
    GeoRow { country: "IT", region: "Lazio",            city: "Rome",          lat: 41.9028, lon:   12.4964, weight:  2 },
    GeoRow { country: "MX", region: "Mexico City",      city: "Mexico City",   lat: 19.4326, lon:  -99.1332, weight:  2 },
    GeoRow { country: "KR", region: "Seoul",            city: "Seoul",         lat: 37.5665, lon:  126.9780, weight:  2 },
    GeoRow { country: "SE", region: "Stockholm",        city: "Stockholm",     lat: 59.3293, lon:   18.0686, weight:  2 },
    GeoRow { country: "PL", region: "Mazovia",          city: "Warsaw",        lat: 52.2297, lon:   21.0122, weight:  2 },
    GeoRow { country: "TR", region: "Istanbul",         city: "Istanbul",      lat: 41.0082, lon:   28.9784, weight:  2 },
    GeoRow { country: "ZA", region: "Gauteng",          city: "Johannesburg",  lat: -26.2041, lon:  28.0473, weight:  1 },
];

const SCREENS_DESKTOP: &[(i64, i64)] = &[
    (1920, 1080), (1366, 768), (1440, 900), (1536, 864), (1680, 1050), (2560, 1440),
];

const SCREENS_MOBILE: &[(i64, i64)] = &[
    (390, 844), (414, 896), (375, 667), (360, 800), (412, 915), (393, 851),
];

const UTM_SOURCES:   &[&str] = &["google", "twitter", "hn", "newsletter", "github", "producthunt"];
const UTM_MEDIUMS:   &[&str] = &["cpc", "social", "email", "referral", "organic"];
const UTM_CAMPAIGNS: &[&str] = &["launch-2026", "spring-promo", "blog-feature", "rebrand", "retarget"];

fn weighted<'a, T>(rng: &mut impl Rng, items: &'a [T], weight: impl Fn(&T) -> u32) -> &'a T {
    let total: u32 = items.iter().map(&weight).sum();
    let mut pick = rng.gen_range(0..total);
    for it in items {
        let w = weight(it);
        if pick < w {
            return it;
        }
        pick -= w;
    }
    items.last().unwrap()
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().collect();
    let sessions: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let days: i64       = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);

    let data_dir = std::env::var("ANALYTICS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"));
    std::fs::create_dir_all(&data_dir)?;

    let pool = db::init(&data_dir).await?;

    let property_id = ensure_property(&pool, PROPERTY_NAME).await?;
    let pid_bytes = property_id.as_bytes().to_vec();

    sqlx::query("DELETE FROM events WHERE property_id = ?")
        .bind(&pid_bytes)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM bot_events WHERE property_id = ?")
        .bind(&pid_bytes)
        .execute(&pool)
        .await?;

    let total = generate(&pool, &pid_bytes, sessions, days).await?;

    println!("Seeded {} sessions ({} events) into property '{}' ({})", sessions, total, PROPERTY_NAME, property_id);
    println!("Dashboard: http://localhost:8000/{}", property_id);

    Ok(())
}

async fn ensure_property(pool: &SqlitePool, name: &str) -> Result<Uuid> {
    let existing: Option<(Vec<u8>,)> = sqlx::query_as("SELECT id FROM properties WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    if let Some((bytes,)) = existing {
        return Ok(Uuid::from_slice(&bytes)?);
    }
    let id = Uuid::new_v4();
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO properties (id, name, custom_cards, is_protected, is_public, created_at, updated_at) \
         VALUES (?, ?, '[]', 0, 0, ?, ?)",
    )
    .bind(id.as_bytes().to_vec())
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn generate(pool: &SqlitePool, pid: &[u8], sessions: usize, days: i64) -> Result<u64> {
    let mut rng = thread_rng();
    let now = Utc::now().timestamp_millis();
    let window_ms: i64 = days * 24 * 60 * 60 * 1000;
    let mut total = 0u64;

    let mut tx = pool.begin().await?;

    for _ in 0..sessions {
        let agent = weighted(&mut rng, AGENTS, |a| a.weight);
        let geo = weighted(&mut rng, GEO, |g| g.weight);
        let referrer_str = weighted(&mut rng, REFERRERS, |r| r.1).0;
        let referrer = if referrer_str.is_empty() { None } else { Some(referrer_str) };

        let user_id = format!("{}", rng.gen_range(100_000_000u64..999_999_999u64));
        let session_start = now - rng.gen_range(0..window_ms);

        let (sw, sh) = if agent.device == "Mobile" {
            *SCREENS_MOBILE.choose(&mut rng).unwrap()
        } else {
            *SCREENS_DESKTOP.choose(&mut rng).unwrap()
        };

        let (utm_source, utm_medium, utm_campaign) = if rng.gen_bool(0.3) {
            (
                Some(*UTM_SOURCES.choose(&mut rng).unwrap()),
                Some(*UTM_MEDIUMS.choose(&mut rng).unwrap()),
                Some(*UTM_CAMPAIGNS.choose(&mut rng).unwrap()),
            )
        } else {
            (None, None, None)
        };

        if agent.is_bot {
            let url_pick = weighted(&mut rng, URLS, |u| u.2);
            sqlx::query(
                "INSERT INTO bot_events (property_id, event, created_at, bot_name, url, user_agent, country, extra) \
                 VALUES (?,?,?,?,?,?,?,'{}')",
            )
            .bind(pid)
            .bind("page_view")
            .bind(session_start)
            .bind(agent.bot_name)
            .bind(url_pick.0)
            .bind(agent.ua)
            .bind(geo.country)
            .execute(&mut *tx)
            .await?;
            total += 1;
            continue;
        }

        let page_count = rng.gen_range(1..=8usize);
        let mut t = session_start;
        let mut url_pick = weighted(&mut rng, URLS, |u| u.2);

        insert_human(&mut tx, pid, "session_start", t, &user_id, url_pick.0, url_pick.1,
                     referrer, agent, sw, sh, geo, utm_source, utm_medium, utm_campaign, None).await?;
        total += 1;

        for i in 0..page_count {
            let time_on_page = rng.gen_range(2_000i64..120_000i64);
            let pv_referrer = if i == 0 { referrer } else { None };

            insert_human(&mut tx, pid, "page_view", t, &user_id, url_pick.0, url_pick.1,
                         pv_referrer, agent, sw, sh, geo, utm_source, utm_medium, utm_campaign, None).await?;
            total += 1;

            if rng.gen_bool(0.4) {
                let click_offset = rng.gen_range(500..time_on_page.max(1001));
                insert_human(&mut tx, pid, "click", t + click_offset, &user_id, url_pick.0, url_pick.1,
                             None, agent, sw, sh, geo, None, None, None, None).await?;
                total += 1;
            }

            insert_human(&mut tx, pid, "page_leave", t + time_on_page, &user_id, url_pick.0, url_pick.1,
                         None, agent, sw, sh, geo, None, None, None, Some(time_on_page)).await?;
            total += 1;

            t += time_on_page + rng.gen_range(500..3000);

            if i + 1 < page_count {
                url_pick = weighted(&mut rng, URLS, |u| u.2);
            }
        }
    }

    tx.commit().await?;
    Ok(total)
}

#[allow(clippy::too_many_arguments)]
async fn insert_human(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    pid: &[u8],
    event: &str,
    created_at: i64,
    user_id: &str,
    url: &str,
    title: &str,
    referrer: Option<&str>,
    agent: &Agent,
    screen_w: i64,
    screen_h: i64,
    geo: &GeoRow,
    utm_source: Option<&str>,
    utm_medium: Option<&str>,
    utm_campaign: Option<&str>,
    time_on_page_ms: Option<i64>,
) -> Result<()> {
    let conn: &mut SqliteConnection = &mut *tx;
    sqlx::query(
        "INSERT INTO events (\
            property_id, event, created_at, user_id, url, title, referrer, user_agent, \
            platform, browser, device, screen_width, screen_height, country, region, city, \
            lat, lon, utm_source, utm_medium, utm_campaign, time_on_page_ms, extra\
         ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'{}')",
    )
    .bind(pid)
    .bind(event)
    .bind(created_at)
    .bind(user_id)
    .bind(url)
    .bind(title)
    .bind(referrer)
    .bind(agent.ua)
    .bind(agent.platform)
    .bind(agent.browser)
    .bind(agent.device)
    .bind(screen_w)
    .bind(screen_h)
    .bind(geo.country)
    .bind(geo.region)
    .bind(geo.city)
    .bind(geo.lat)
    .bind(geo.lon)
    .bind(utm_source)
    .bind(utm_medium)
    .bind(utm_campaign)
    .bind(time_on_page_ms)
    .execute(conn)
    .await?;
    Ok(())
}
