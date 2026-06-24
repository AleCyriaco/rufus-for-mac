# Rufus for Mac

A macOS take on [Rufus](https://github.com/pbatard/rufus) — the bootable-USB
creator. **Rust core** (lists disks via `diskutil`, writes via `dd`, asks for admin
rights via `osascript`) wrapped in a **Tauri** window styled after the original Rufus.

![platform](https://img.shields.io/badge/platform-macOS-black)
![language](https://img.shields.io/badge/core-Rust-orange)
![ui](https://img.shields.io/badge/ui-Tauri%20v2-24C8DB)
![license](https://img.shields.io/badge/license-MIT-green)

> **Do you even need this?** For most images, `sudo dd if=image.iso of=/dev/rdiskN bs=4m`
> already does the job on macOS — that's why Rufus never existed here (Windows lacks a
> built-in `dd`). This app is that one-liner with a Rufus-style face, device picker, and
> guardrails.

---

## Features

| | Status |
|---|---|
| List external USB devices (name + size) | ✅ done |
| Pick an image (`.iso` / `.img` / `.dmg`) | ✅ done |
| Write to device with `dd` (native admin prompt) | ✅ done |
| English UI, Portuguese optional (toggle, remembered) | ✅ done |
| Rufus-style layout & options | ✅ done |
| Live byte progress | ⏳ indeterminate bar for now — see [Roadmap](#roadmap) |
| Windows ISO support (FAT32 + `install.wim` split) | ⛔ not yet |
| Partition scheme / file system / cluster size | 🎚️ visual only (stubs) |

## Requirements

- macOS (uses the built-in `diskutil`, `dd`, `osascript`)
- [Rust](https://rustup.rs)
- Tauri CLI v2: `cargo install tauri-cli --version "^2.0"`

## Run

```sh
git clone https://github.com/AleCyriaco/rufus-for-mac.git
cd rufus-for-mac/src-tauri
cargo tauri dev
```

The window opens in English. There is no separate build step — `cargo tauri dev`
compiles the Rust core and launches the app.

## Usage

1. Plug in the USB drive.
2. **Device** — pick it from the dropdown (`↻ Refresh` to rescan). The label shows
   name, size, and `/dev/diskN`.
3. **Boot selection** — `SELECT` and choose your `.iso` / `.img` / `.dmg`.
4. **START** — macOS asks for your admin password (that prompt is the real safety gate).
   The drive is unmounted, written with `dd`, then ejected.

> ⚠️ **Writing erases everything on the selected device.** Double-check the disk in the
> dropdown before you start.

### Languages

UI defaults to **English**. Toggle **EN · PT** (top-right) for Portuguese; the choice is
saved in `localStorage`. Strings live in the `T` dictionary in `ui/index.html`.

## Architecture

```
ui/index.html          UI — HTML + CSS + JS inline, no bundler, withGlobalTauri
src-tauri/src/main.rs   Rust core — 3 commands, all shell-outs:
                          list_disks  → diskutil list/info -plist  (parsed with the `plist` crate)
                          pick_image  → osascript "choose file"
                          flash       → diskutil unmount + dd (via osascript admin) + eject
src-tauri/tauri.conf.json   480×680 fixed window, frontendDist = ../ui
```

Design choices (kept deliberately lazy):

- **No IOKit `unsafe` FFI.** `diskutil` already returns everything as plist.
- **No npm / bundler.** `withGlobalTauri` exposes `window.__TAURI__`; the UI is one static file.
- **Privilege via the native admin prompt.** Typing your password *is* the confirmation step.

## Build a `.app` / `.dmg`

```sh
cargo tauri icon path/to/icon.png      # generates src-tauri/icons/* (1024×1024 PNG source)
```
Then in `src-tauri/tauri.conf.json` set `bundle.active = true` and `bundle.targets = "dmg"`:
```sh
cargo tauri build
```
The repo ships a plain green placeholder `icon.png` so `cargo tauri dev` runs out of the box.

## Roadmap

- [ ] **Live byte progress.** `osascript ... with administrator privileges` only returns
      when `dd` finishes, so the bar is indeterminate today. Upgrade path: a privileged
      helper (`SMAppService`) launching `dd` and polling `SIGINFO` for real progress.
- [ ] **Windows ISO support.** FAT32 can't hold an `install.wim` > 4 GB; needs splitting.
      Don't reinvent it — wire up `wimlib` (`brew install wimlib`).
- [ ] **Real format options.** Partition scheme / file system / cluster size are visual
      stubs (`dd` writes the image byte-for-byte, so they don't apply yet).
- [ ] System-language auto-detection (`navigator.language`) on top of the manual toggle.

Shortcuts in the code are tagged with `// cyrix:` comments naming the ceiling and the fix.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). TL;DR: keep it small, prefer native tools over new
crates, one runnable check per non-trivial change.

## Credits & legal

- Inspired by [Rufus](https://github.com/pbatard/rufus) by Pete Batard.
- This is an **independent reimplementation in Rust** — it contains **no source code** from
  the original Rufus (which is GPLv3). "Rufus" is a trademark of its author; this project is
  an unaffiliated homage.

## License

[MIT](LICENSE) © 2026 Ale Cyriaco
