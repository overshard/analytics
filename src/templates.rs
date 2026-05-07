use minijinja::value::{Kwargs, Value};
use minijinja::{path_loader, AutoEscape, Environment, Error, ErrorKind, Output, State};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::path::Path;

/// Jinja2-faithful HTML formatter — does NOT escape `/`, so vite asset URLs
/// like `/static/base-abc123.js` come through clean instead of `&#x2f;...`.
fn jinja2_html_formatter(out: &mut Output, state: &State, value: &Value) -> Result<(), Error> {
    if value.is_safe() {
        write!(out, "{value}").map_err(Error::from)?;
        return Ok(());
    }
    let auto_escape = match state.auto_escape() {
        AutoEscape::Html => true,
        AutoEscape::None => false,
        _ => return minijinja::escape_formatter(out, state, value),
    };
    if !auto_escape {
        write!(out, "{value}").map_err(Error::from)?;
        return Ok(());
    }
    if let Some(s) = value.as_str() {
        write_jinja2_html(out, s).map_err(Error::from)?;
    } else if value.is_undefined() || value.is_none() {
        // emit nothing
    } else {
        let stringified = value.to_string();
        write_jinja2_html(out, &stringified).map_err(Error::from)?;
    }
    Ok(())
}

fn write_jinja2_html(out: &mut Output, s: &str) -> std::fmt::Result {
    let mut last = 0;
    for (i, b) in s.bytes().enumerate() {
        let escape = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&#34;",
            b'\'' => "&#39;",
            _ => continue,
        };
        if last < i {
            out.write_str(&s[last..i])?;
        }
        out.write_str(escape)?;
        last = i + 1;
    }
    if last < s.len() {
        out.write_str(&s[last..])?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestCtx {
    pub url: String,
    pub url_root: String,
    pub base_url: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UserCtx {
    pub is_authenticated: bool,
}

fn read_manifest(path: &Path) -> JsonValue {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".to_string());
    serde_json::from_str(&text).unwrap_or(JsonValue::Null)
}

fn lookup_asset(manifest: &JsonValue, entry: &str, kind: &str) -> String {
    if let Some(chunk) = manifest.get(entry) {
        if kind == "css" {
            if let Some(css_arr) = chunk.get("css").and_then(|v| v.as_array()) {
                if let Some(first) = css_arr.first().and_then(|v| v.as_str()) {
                    return format!("/static/{first}");
                }
            }
        }
        if let Some(file) = chunk.get("file").and_then(|v| v.as_str()) {
            return format!("/static/{file}");
        }
    }
    format!("/static/{entry}")
}

pub fn build_env(templates_dir: &Path, manifest_path: &Path) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(templates_dir));
    env.set_formatter(jinja2_html_formatter);

    #[cfg(debug_assertions)]
    {
        let path = manifest_path.to_path_buf();
        env.add_function(
            "vite_asset",
            move |entry: String, kind: Option<String>| -> Result<String, Error> {
                let kind = kind.unwrap_or_else(|| "file".to_string());
                let manifest = read_manifest(&path);
                Ok(lookup_asset(&manifest, &entry, &kind))
            },
        );
    }
    #[cfg(not(debug_assertions))]
    {
        let manifest = read_manifest(manifest_path);
        env.add_function(
            "vite_asset",
            move |entry: String, kind: Option<String>| -> Result<String, Error> {
                let kind = kind.unwrap_or_else(|| "file".to_string());
                Ok(lookup_asset(&manifest, &entry, &kind))
            },
        );
    }

    env.add_function("url_for", url_for);
    env.add_filter("naturaltime", naturaltime_filter);
    env.add_filter("urlencode", urlencode_filter);

    env
}

fn urlencode_filter(value: Value) -> Result<String, Error> {
    let s = value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string());
    Ok(urlencoding::encode(&s).into_owned())
}

/// Subset of Django's url_for/url tags. We only need to emit a URL string,
/// so we only support the names referenced by templates.
fn url_for(_state: &State, endpoint: String, kwargs: Kwargs) -> Result<String, Error> {
    let take_str = |k: &str| -> Result<Option<String>, Error> {
        let v: Option<Value> = kwargs.get(k).ok();
        match v {
            None => Ok(None),
            Some(val) => {
                if val.is_undefined() || val.is_none() {
                    Ok(None)
                } else {
                    Ok(Some(val.to_string()))
                }
            }
        }
    };

    let path = match endpoint.as_str() {
        "home" | "index" => "/".to_string(),
        "login" => "/login".to_string(),
        "logout" => "/logout".to_string(),
        "properties" => "/properties".to_string(),
        "property" => {
            let id = take_str("property_id")?.unwrap_or_default();
            format!("/{id}")
        }
        "property_delete" => {
            let id = take_str("property_id")?.unwrap_or_default();
            format!("/properties/{id}/delete")
        }
        "property_cards" => {
            let id = take_str("property_id")?.unwrap_or_default();
            format!("/properties/{id}/cards")
        }
        "property_public" => {
            let id = take_str("property_id")?.unwrap_or_default();
            format!("/properties/{id}/public")
        }
        "documentation" => "/documentation".to_string(),
        "changelog" => "/changelog".to_string(),
        "favicon" => "/favicon.ico".to_string(),
        "static" => {
            let filename = take_str("filename")?.unwrap_or_default();
            format!("/static/{filename}")
        }
        other => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("unknown route in url_for: {other}"),
            ));
        }
    };
    kwargs.assert_all_used()?;
    Ok(path)
}

/// Mimics Django's humanize "naturaltime" for createdAt timestamps.
fn naturaltime_filter(value: Value) -> Result<String, Error> {
    let s = value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string());
    let dt = chrono::DateTime::parse_from_rfc3339(&s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok();
    let Some(dt) = dt else { return Ok(s) };
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);
    let secs = diff.num_seconds();
    Ok(if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let m = secs / 60;
        format!("{m} minute{} ago", if m == 1 { "" } else { "s" })
    } else if secs < 86_400 {
        let h = secs / 3600;
        format!("{h} hour{} ago", if h == 1 { "" } else { "s" })
    } else if secs < 86_400 * 30 {
        let d = secs / 86_400;
        format!("{d} day{} ago", if d == 1 { "" } else { "s" })
    } else if secs < 86_400 * 365 {
        let m = secs / (86_400 * 30);
        format!("{m} month{} ago", if m == 1 { "" } else { "s" })
    } else {
        let y = secs / (86_400 * 365);
        format!("{y} year{} ago", if y == 1 { "" } else { "s" })
    })
}
