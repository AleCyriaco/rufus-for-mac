#!/usr/bin/env python3
"""Renderiza ui/index.html via Chrome headless pra um PNG de divulgação.
Stuba window.__TAURI__ (device + imagem fake) e força dark mode, já que num browser
puro o app mostraria erro de IPC. cyrix: nada de duplicar markup — injeta no html real.
Uso: python3 scripts/make_screenshot.py  → docs/screenshot.png
"""
import os
import subprocess
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
html = open(os.path.join(ROOT, "ui", "index.html")).read()

STUB = """<script>
window.__TAURI__ = {
  core: { invoke: async (cmd) => {
    if (cmd === 'list_disks') return [{ id: 'disk4', name: 'SanDisk Ultra', size: 30800000000 }];
    if (cmd === 'pick_image') return '/Users/Ale/Desktop/Win11_25H2_English_x64_v2.iso';
    return null;
  } },
  event: { listen: async () => () => {} }
};
</script>"""
DARK = """<style>:root{--bg:#26262b;--panel:#303036;--line:#4a4a52;--ink:#f2f2f7;
--muted:#a1a1a6;--field:#1d1d22;--accent:#15a64a;--accent-press:#128a3f;--warn:#ff5c5c;}</style>"""
POST = """<script>setTimeout(() => document.getElementById('select').click(), 250);</script>"""

html = html.replace("<head>", "<head>\n" + STUB, 1)
html = html.replace("</head>", DARK + "\n</head>", 1)
html = html.replace("</body>", POST + "\n</body>", 1)

tmp = os.path.join(tempfile.gettempdir(), "rufus-shot.html")
open(tmp, "w").write(html)

os.makedirs(os.path.join(ROOT, "docs"), exist_ok=True)
out = os.path.join(ROOT, "docs", "screenshot.png")
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
subprocess.run([
    chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars",
    "--force-device-scale-factor=2", "--window-size=480,892",
    "--virtual-time-budget=2500", f"--screenshot={out}", f"file://{tmp}",
], check=True)
print("wrote", out)
