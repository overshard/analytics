"""
Utilities for generating screenshots and PDFs using Playwright.
"""
from __future__ import annotations

import os
import tempfile
from contextlib import contextmanager
from typing import Generator, Iterable, Optional

from django.core.files import File
from django.core.files.storage import default_storage
from playwright.sync_api import Page, sync_playwright

DEFAULT_VIEWPORT = {"width": 1280, "height": 720}
DEFAULT_LAUNCH_ARGS = (
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--disable-gpu",
    "--disable-setuid-sandbox",
    "--disable-software-rasterizer",
    "--disable-extensions",
    "--disable-background-networking",
)


def _env_flag(name: str, default: bool = True) -> bool:
    value = os.environ.get(name)
    if value is None:
        return default
    return value.strip().lower() not in {"0", "false", "no"}


def _env_int(name: str, default: int) -> int:
    value = os.environ.get(name)
    if value is None:
        return default
    try:
        return int(value)
    except ValueError:
        return default


def _env_float(name: str, default: float) -> float:
    value = os.environ.get(name)
    if value is None:
        return default
    try:
        return float(value)
    except ValueError:
        return default


def _collect_launch_args() -> Iterable[str]:
    extra = os.environ.get("PLAYWRIGHT_LAUNCH_ARGS", "")
    return list(DEFAULT_LAUNCH_ARGS) + [arg for arg in extra.split() if arg]


@contextmanager
def _page_context(viewport: Optional[dict] = None) -> Generator[Page, None, None]:
    viewport = viewport or DEFAULT_VIEWPORT
    navigation_timeout = _env_int("PLAYWRIGHT_NAVIGATION_TIMEOUT", 45_000)
    action_timeout = _env_int("PLAYWRIGHT_ACTION_TIMEOUT", navigation_timeout)

    launch_kwargs = {
        "headless": _env_flag("PLAYWRIGHT_HEADLESS", True),
        "args": list(_collect_launch_args()),
    }

    executable = os.environ.get("PLAYWRIGHT_CHROMIUM_EXECUTABLE")
    if executable:
        launch_kwargs["executable_path"] = executable

    channel = os.environ.get("PLAYWRIGHT_CHROMIUM_CHANNEL")
    if channel:
        launch_kwargs["channel"] = channel

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(**launch_kwargs)
        context = browser.new_context(
            viewport=viewport,
            device_scale_factor=_env_float("PLAYWRIGHT_DEVICE_SCALE_FACTOR", 1.0),
        )
        page = context.new_page()
        page.set_default_navigation_timeout(navigation_timeout)
        page.set_default_timeout(action_timeout)
        try:
            yield page
        finally:
            context.close()
            browser.close()


def _temporary_file(suffix: str) -> str:
    handle, path = tempfile.mkstemp(suffix=suffix)
    os.close(handle)
    return path


def _save_and_cleanup(tempfilename: str, filename: str) -> str:
    if default_storage.exists(filename):
        default_storage.delete(filename)
    with open(tempfilename, "rb") as file_pointer:
        default_storage.save(filename, File(file_pointer))
    os.remove(tempfilename)
    return default_storage.url(filename)


def save_tempfile_to_storage(tempfilename: str, filename: str) -> str:
    """
    Backwards compatibility wrapper for the old chromium module API.
    """
    return _save_and_cleanup(tempfilename, filename)


def _load_url(page: Page, url: str) -> None:
    page.goto(url, wait_until="networkidle")


def _load_html(page: Page, html: str) -> None:
    temp_html_path = _temporary_file(".html")
    try:
        with open(temp_html_path, "w", encoding="utf-8") as temp_file:
            temp_file.write(html)
        page.goto(f"file://{temp_html_path}", wait_until="networkidle")
    finally:
        if os.path.exists(temp_html_path):
            os.remove(temp_html_path)


def generate_screenshot_from_url(url: str, filename: str) -> str:
    tempfilename = _temporary_file(".png")
    with _page_context() as page:
        _load_url(page, url)
        page.screenshot(path=tempfilename, full_page=True)
    return _save_and_cleanup(tempfilename, filename)


def generate_screenshot_from_html(html: str, filename: str) -> str:
    tempfilename = _temporary_file(".png")
    with _page_context() as page:
        _load_html(page, html)
        page.screenshot(path=tempfilename, full_page=True)
    return _save_and_cleanup(tempfilename, filename)


def generate_pdf_from_url(url: str, filename: str) -> str:
    tempfilename = _temporary_file(".pdf")
    with _page_context() as page:
        _load_url(page, url)
        page.pdf(
            path=tempfilename,
            format=os.environ.get("PLAYWRIGHT_PDF_FORMAT", "A4"),
            print_background=_env_flag("PLAYWRIGHT_PDF_PRINT_BACKGROUND", True),
            prefer_css_page_size=_env_flag("PLAYWRIGHT_PDF_PREFER_CSS_PAGE_SIZE", True),
        )
    return _save_and_cleanup(tempfilename, filename)


def generate_pdf_from_html(html: str, filename: str) -> str:
    tempfilename = _temporary_file(".pdf")
    with _page_context() as page:
        _load_html(page, html)
        page.pdf(
            path=tempfilename,
            format=os.environ.get("PLAYWRIGHT_PDF_FORMAT", "A4"),
            print_background=_env_flag("PLAYWRIGHT_PDF_PRINT_BACKGROUND", True),
            prefer_css_page_size=_env_flag("PLAYWRIGHT_PDF_PREFER_CSS_PAGE_SIZE", True),
        )
    return _save_and_cleanup(tempfilename, filename)
