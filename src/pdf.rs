use std::path::PathBuf;
use std::process::Command;
use tempfile::{Builder, NamedTempFile};

/// Render an HTML string to a PDF using headless Chromium.
///
/// Spawns chrome-headless-shell with --print-to-pdf. The HTML is written to a
/// tempfile and loaded as a file:// URL so relative URLs (e.g. /content/images/...)
/// can be rewritten to absolute http:// URLs pointing at the live server.
pub fn html_to_pdf(html: &str, server_base: &str) -> anyhow::Result<Vec<u8>> {
    // Rewrite relative absolute paths (/content/, /static/) to point at the
    // running server so chromium can fetch them.
    let rewritten = html
        .replace("src=\"/content/", &format!("src=\"{server_base}/content/"))
        .replace("href=\"/content/", &format!("href=\"{server_base}/content/"))
        .replace("src=\"/static/", &format!("src=\"{server_base}/static/"))
        .replace("href=\"/static/", &format!("href=\"{server_base}/static/"));

    // Suffix matters: chromium decides HTML vs plain-text rendering by extension.
    // Without .html the page is shown as raw source text instead of being rendered.
    let mut html_file = Builder::new().suffix(".html").tempfile()?;
    {
        use std::io::Write;
        html_file.write_all(rewritten.as_bytes())?;
        html_file.flush()?;
    }
    let html_path = html_file.path().to_path_buf();
    let pdf_file = NamedTempFile::new()?;
    let pdf_path = pdf_file.path().to_path_buf();

    let url = format!("file://{}", html_path.display());
    let print_arg = format!("--print-to-pdf={}", pdf_path.display());

    run_chromium(&url, &print_arg)?;

    let bytes = std::fs::read(&pdf_path)?;
    Ok(bytes)
}

fn run_chromium(url: &str, print_arg: &str) -> anyhow::Result<()> {
    let bin = find_chromium().ok_or_else(|| {
        anyhow::anyhow!("could not locate chromium; set CHROMIUM_BIN or install chromium on PATH")
    })?;
    let output = Command::new(&bin)
        .arg("--headless=new")
        .arg("--no-sandbox")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg("--hide-scrollbars")
        .arg(print_arg)
        .arg(url)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "chromium ({}) exited {}: {}",
            bin.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Locate a chromium binary across environments:
/// 1. `CHROMIUM_BIN` env var (explicit override)
/// 2. PATH search for common chromium binary names (production install)
/// 3. Glob under `/opt/playwright-browsers/` (dev container with Playwright)
fn find_chromium() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHROMIUM_BIN") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }

    let names = [
        "chromium",
        "chromium-browser",
        "chrome-headless-shell",
        "google-chrome",
        "chrome",
    ];
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in &names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir("/opt/playwright-browsers") {
        for entry in entries.flatten() {
            let candidate = entry
                .path()
                .join("chrome-headless-shell-linux64/chrome-headless-shell");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
