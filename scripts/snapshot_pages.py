#!/usr/bin/env python3
"""Render every (page, theme) combination of docs/prototype.html as PNG.

Output: docs/previews/{theme}/{page}.png  (18 pages × 3 themes = 54 PNGs)

Strategy:
  - Single browser context reused across runs (fast, real CSS transitions)
  - For each (theme, page):
      1. set localStorage theme → reload to apply CSS variables cleanly
      2. set hash to #<page> → route() reads it and renders
      3. wait 200ms for any async (lucide icon font load, etc.)
      4. screenshot to docs/previews/{theme}/{page}.png
  - For onboarding (first-run-only), we explicitly set the route via JS.

Why reload: setting data-theme on <body> only updates CSS variables that
are already in the cascade. Reload guarantees the new theme is the one
that drove the first paint, so any pre-paint rendering matches the PNG.
"""

import os
import sys
import time
from pathlib import Path
from playwright.sync_api import sync_playwright

PAGES = [
    "dashboard",
    "markets",
    "market-detail",
    "signals",
    "copy",
    "pnl",
    "history",
    "lab",
    "analysis",
    "llm-perf",
    "llm-mgmt",
    "brief",
    "wallets",
    "notifications",
    "audit",
    "help",
    "settings",
    "onboarding",
]
THEMES = ["dark", "light", "matrix"]
ROOT = Path("/Users/dutongxue/work2/polyrocket")
HTML = ROOT / "docs/prototype.html"
OUT = ROOT / "docs/previews"

VIEWPORT = {"width": 1440, "height": 900}

# First-run guard: prototype.html routes to onboarding if localStorage
# `polyrocket.first-run-done` is not "1". We force it on for non-onboarding
# pages, and force it off for the onboarding screenshot.
JS_PRELUDE = """
() => {
  window.scrollTo(0, 0);
}
"""


def main() -> int:
    if not HTML.exists():
        print(f"ERR: {HTML} not found", file=sys.stderr)
        return 1
    for theme in THEMES:
        (OUT / theme).mkdir(parents=True, exist_ok=True)
    failures = []
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(viewport=VIEWPORT, device_scale_factor=2)
        # Pre-set localStorage so we don't get bounced to onboarding when
        # the page first loads. We override per-page below.
        context.add_init_script("""
            try { localStorage.setItem('polyrocket.first-run-done', '1'); } catch(e) {}
        """)
        page = context.new_page()
        # Capture console + page errors for debugging
        captured = {"msgs": []}
        def _on_console(msg):
            captured["msgs"].append(f"[{msg.type}] {msg.text}")
        def _on_pageerror(err):
            captured["msgs"].append(f"[pageerror] {err}")
        page.on("console", _on_console)
        page.on("pageerror", _on_pageerror)
        for theme in THEMES:
            for route_name in PAGES:
                is_onboarding = route_name == "onboarding"
                first_run = "0" if is_onboarding else "1"
                # 1. seed localStorage by visiting the file once with a tiny
                #    page that does the writes and then redirects to the hash.
                #    This is more reliable than add_init_script because we
                #    know the writes complete before any of the prototype's
                #    own scripts run.
                seed_html = (
                    "<!doctype html><meta charset=utf-8>"
                    "<script>"
                    f"localStorage.setItem('polyrocket.theme','{theme}');"
                    f"localStorage.setItem('polyrocket.first-run-done','{first_run}');"
                    f"localStorage.setItem('polyrocket.sidebar.collapsed','0');"
                    f"location.replace('file://{HTML}#{route_name}');"
                    "</script>"
                )
                seed_path = ROOT / "docs" / ".seed.html"
                seed_path.write_text(seed_html, encoding="utf-8")
                try:
                    page.goto(f"file://{seed_path}", wait_until="load")
                except Exception as e:
                    print(f"ERR goto-seed {theme}/{route_name}: {e}", file=sys.stderr)
                    failures.append((theme, route_name, str(e)))
                    continue
                # 2. wait for the prototype's own applyTheme() to have run
                try:
                    page.wait_for_function(
                        f"document.documentElement.getAttribute('data-theme') === '{theme}'",
                        timeout=5000,
                    )
                except Exception as e:
                    print(f"ERR theme-set {theme}/{route_name}: {e}", file=sys.stderr)
                    failures.append((theme, route_name, f"theme: {e}"))
                    continue
                # 3. wait for lucide icons
                try:
                    page.wait_for_function(
                        "typeof window.lucide !== 'undefined' && document.querySelectorAll('[data-lucide]').length > 0",
                        timeout=5000,
                    )
                except Exception:
                    pass  # not all pages have data-lucide; tolerate
                # 4. extra settle for any in-flight route() / skeleton paints
                page.wait_for_timeout(300)
                # DEBUG: peek at content + body to diagnose empty screenshots
                if (THEMES.index(theme), PAGES.index(route_name)) == (0, 0):
                    print(f"  DEBUG console ({len(captured['msgs'])} msgs): {captured['msgs'][:10]}")
                    captured["msgs"].clear()
                    try:
                        snippet = page.evaluate("""() => {
                          const c = document.getElementById('content');
                          return {
                            theme: document.documentElement.getAttribute('data-theme'),
                            contentLen: c ? c.innerHTML.length : -1,
                            contentFirst: c ? c.innerHTML.slice(0, 200) : '',
                            hash: location.hash,
                            h1: (document.querySelector('h1') || {}).textContent || '',
                            bcCurrent: (document.getElementById('bcCurrent') || {}).textContent || '',
                            ROUTES: typeof ROUTES,
                            renderDashboardLen: (typeof renderDashboard === 'function') ? renderDashboard().length : -1,
                          };
                        }""")
                        print(f"  DEBUG {theme}/{route_name}: {snippet}")
                    except Exception as e:
                        print(f"  DEBUG eval err: {e}")
                # 5. screenshot full page
                out_path = OUT / theme / f"{route_name}.png"
                try:
                    page.screenshot(path=str(out_path), full_page=True)
                    print(f"  ok   {theme}/{route_name}.png")
                except Exception as e:
                    print(f"ERR shot {theme}/{route_name}: {e}", file=sys.stderr)
                    failures.append((theme, route_name, str(e)))
        browser.close()
    if failures:
        print(f"\n{len(failures)} failure(s):", file=sys.stderr)
        for t, r, e in failures:
            print(f"  - {t}/{r}: {e}", file=sys.stderr)
        return 1
    print(f"\nOK — {len(PAGES) * len(THEMES)} PNGs written to {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
