use chrono::Datelike;
use maxminddb::geoip2;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct GeoIp {
    path: PathBuf,
    reader: RwLock<Option<maxminddb::Reader<Vec<u8>>>>,
}

#[derive(Debug, Clone, Default)]
pub struct GeoLookup {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

impl GeoIp {
    pub fn load(path: &Path) -> Self {
        let reader = maxminddb::Reader::open_readfile(path).ok();
        if reader.is_some() {
            tracing::info!("geoip db loaded from {}", path.display());
        } else {
            tracing::warn!(
                "geoip db missing at {} — country/region enrichment disabled until refresh",
                path.display()
            );
        }
        Self {
            path: path.to_path_buf(),
            reader: RwLock::new(reader),
        }
    }

    pub fn reload(&self) -> bool {
        let new_reader = maxminddb::Reader::open_readfile(&self.path).ok();
        let ok = new_reader.is_some();
        if let Ok(mut w) = self.reader.write() {
            *w = new_reader;
        }
        ok
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<GeoLookup> {
        let guard = self.reader.read().ok()?;
        let reader = guard.as_ref()?;
        let city: geoip2::City = reader.lookup(ip).ok()?;
        let country = city
            .country
            .as_ref()
            .and_then(|c| c.iso_code.as_ref().map(|s| s.to_string()));
        let region = city
            .subdivisions
            .as_ref()
            .and_then(|subs| subs.first())
            .and_then(|s| {
                s.names
                    .as_ref()
                    .and_then(|n| n.get("en").map(|v| v.to_string()))
                    .or_else(|| s.iso_code.map(|s| s.to_string()))
            });
        let city_name = city
            .city
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|n| n.get("en").map(|v| v.to_string()));
        let (lat, lon) = city
            .location
            .as_ref()
            .map(|l| (l.latitude, l.longitude))
            .unwrap_or((None, None));

        Some(GeoLookup { country, region, city: city_name, lat, lon })
    }
}

/// Download the latest DB-IP City Lite mmdb to `dest` if missing or older than 30 days.
/// CC-BY-4.0, no signup required.
///
/// DB-IP rolls each month's file on the 1st, but with a few hours of lag.
/// Try this month, last month, then two months back so a first-of-the-month
/// boot doesn't 404 us into a degraded state.
pub async fn ensure_db(dest: &Path) -> anyhow::Result<bool> {
    if dest.exists() {
        if let Ok(meta) = std::fs::metadata(dest) {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age.as_secs() < 30 * 24 * 60 * 60 {
                    return Ok(false);
                }
            }
        }
    }

    let today = chrono::Utc::now().date_naive();
    let mut last_err: Option<anyhow::Error> = None;
    for offset in 0i64..3 {
        let target = month_offset(today, offset);
        let url = format!(
            "https://download.db-ip.com/free/dbip-city-lite-{}-{:02}.mmdb.gz",
            target.year(),
            target.month()
        );
        match download_gz_to(&url, dest).await {
            Ok(()) => {
                tracing::info!("downloaded geoip db from {url}");
                return Ok(true);
            }
            Err(e) => {
                tracing::warn!(
                    "geoip download failed for {}-{:02}: {e}",
                    target.year(),
                    target.month()
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("geoip download failed (no candidates)")))
}

/// Return the first-of-the-month for `today` shifted back `offset` months.
fn month_offset(today: chrono::NaiveDate, offset: i64) -> chrono::NaiveDate {
    let mut y = today.year();
    let mut m = today.month() as i64 - offset;
    while m <= 0 {
        m += 12;
        y -= 1;
    }
    chrono::NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(today)
}

async fn download_gz_to(url: &str, dest: &Path) -> anyhow::Result<()> {
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, out)?;
    Ok(())
}
