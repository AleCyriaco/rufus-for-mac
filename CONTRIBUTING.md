# Contributing

Thanks for considering a contribution. This project is intentionally small —
it shells out to native macOS tools instead of reimplementing disk I/O — so
keep changes lean.

## Setup

```sh
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tauri CLI v2
cargo install tauri-cli --version "^2.0"
```

## Develop

```sh
cd src-tauri
cargo tauri dev      # launches the app with live reload
cargo test           # runs the disk-parsing unit tests
```

- **Rust core** lives in `src-tauri/src/main.rs` — three commands: `list_disks`,
  `pick_image`, `flash`. All of them shell out to `diskutil` / `dd` / `osascript`.
- **UI** lives in `ui/index.html` — plain HTML/CSS/JS, no bundler, no npm. Strings
  are in the `T` dictionary (English + Portuguese); add new keys to both.

## Style

- Prefer native macOS tools over new crates or `unsafe` FFI.
- Keep the diff small. Deletion beats addition.
- Deliberate shortcuts are marked with `// cyrix:` comments naming the ceiling and
  the upgrade path. If you lift one, remove the comment.
- Non-trivial logic gets one runnable check (see the tests in `main.rs`).

## Safety

`flash` writes a raw image to a block device with `dd` — it **erases the target**.
Any change there must keep the device validation and the admin-password prompt intact.

## PRs

Small, focused PRs with a one-line description of what and why. If you touch the
roadmap items in the README, update that checklist too.
