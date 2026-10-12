# RarZap — Technical Stack

> Scope: the open-source tree in this repository. The commercial batch plugin
> lives outside the tree (local `plugins/`, git-ignored) and is out of scope here.

## 1. Overview

RarZap is a small, native desktop app that converts **RAR archives to ZIP**,
entirely on the local machine. RAR is a proprietary, read-only format, so the
pipeline is: **extract RAR → repack as ZIP**.

| Layer | Choice | Notes |
|---|---|---|
| UI framework | [GPUI Kit](https://gpui-kit.com) (`gpui-kit` + `gpui-component`) | GPU-native, no WebView |
| RAR extraction | `unrar-ng` | Fast batch extraction API |
| ZIP packing | `zip` | Deflate; zlib-rs backend |
| File dialogs | GPUI `prompt_for_paths()` | Native OS pickers |
| Errors | `anyhow` (app) + `thiserror` (lib) | Layered error handling |
| Logging | `tracing` + `tracing-appender` | Structured, daily rotation |
| i18n | `rust-i18n` + `sys-locale` | 8 locales, compiled-in YAML |
| Config | `serde` + `serde_json` + `directories` | JSON under the OS config dir |
| Toolchain | Rust **1.92+** (edition 2024) | Required by Linux deps |

## 2. UI layer

### 2.1 GPUI / GPUI Kit

GPUI is the Rust-native UI framework behind the Zed editor. It renders directly
on the GPU (Metal on macOS, DirectX on Windows, Vulkan on Linux) and compiles to
a plain native binary — **no WebView, no Electron**.

RarZap depends on **GPUI Kit** (`gpui-kit`), a facade that bundles:

- `gpui` — the rendering/layout core
- `gpui-base` — styling/state primitives
- `gpui-component` — a library of styled desktop widgets

The app must start through `gpui-kit` (it wires the platform backend and asset
source). Trade-offs are the usual pre-1.0 ones: the API can change between
minor versions, docs are thin in places, and Linux pulls in system libraries
(`libxcb`, `libxkbcommon`, Wayland/X11, Vulkan).

### 2.2 Components

`gpui-component` provides the widgets RarZap builds on: `Button`,
`Progress`, `Input`, `Dialog`/modal overlays, `TitleBar`, and more. Surfaces
that are not stock widgets (the empty-state drop zone, summary cards) are
composed from GPUI's flexbox `div()` primitives.

## 3. Archive layer

### 3.1 RAR extraction — `unrar-ng`

`unrar-ng` is an actively maintained fork of the `unrar` crate. Its key addition
is a **batch extraction API** (`extract_all_with_callback`) that uses the same
single-pass loop as the native `unrar` CLI, removing the redundant per-file
header traversal overhead of the original crate. This matters a lot for batch
conversion. A callback stream reports per-file start/ok/error events (with byte
sizes), which drives the progress bar.

Version note: 0.7 renamed the library to `unrar_ng` in `use` statements.

### 3.2 ZIP packing — `zip`

The `zip` crate is the standard Rust ZIP library (PKWARE APPNOTE). RarZap uses
**Deflate** for maximum compatibility — Windows Explorer, macOS Archive Utility,
and Linux unzip all read it natively. The effective Deflate backend is
**zlib-rs** (SIMD). Compression level is configurable (1–9) and overridable via
the `RARZAP_ZIP_LEVEL` environment variable; the default keeps the crate's
level 6.

### 3.3 Conversion pipeline and safety

`src/core/converter.rs` is the single-file primitive:

1. **List** the archive headers only (no disk writes) to total the unpacked
   bytes/files and to pre-screen entry names (reject absolute paths, `..`,
   drive letters, ADS `file:stream`, and Windows-style `..\` traversal —
   normalized across platforms).
2. **Extract** to a staging dir on the *same disk* as the target, so the final
   move can be an atomic rename.
3. **Verify** the staging tree in a single pass: no symlinks, every path lexically
   contained, actual byte/file counts re-checked against the limits — while also
   collecting the entry list for packing.
4. **Pack** to `output.zip` inside staging, then **rename** it into place.

Guarantees and limits:

- Output is **atomic** — the `.zip` only appears on success; cancel/failure
  leaves no partial file.
- **No overwrite** by default; callers may generate `name (2).zip` style
  collision-free names.
- **Zip-bomb limits**: default cap of 20 GiB unpacked and 100k files, enforced
  at header-scan, during extraction callbacks, and after extraction
  (`RARZAP_MAX_BYTES` / `RARZAP_MAX_FILES` override).
- Suspicious archives (unsafe entry names, symlinks) are rejected, not silently
  repaired.

## 4. File dialogs & system integration

GPUI exposes native dialogs; RarZap never implements its own picker:

- `prompt_for_paths()` — open dialog (single/multi), returns a future awaited
  with `cx.spawn()`.
- `reveal_path()` — reveal a file in Finder / Explorer / Nautilus.
- `open_url()` — open the buy page in the default browser.

On Linux, dialogs go through the XDG Desktop Portal, matching GNOME/KDE.

## 5. Errors & logging

- **Application layer** uses `anyhow::Result` with `.with_context()` for
  human-readable chains.
- **Library/module boundaries** (the converter core) define typed errors with
  `thiserror` (`src/core/error.rs`).

Logging uses `tracing` with a daily-rotated file appender under the OS data
directory (`%LOCALAPPDATA%/RarZap/data/logs`, `~/Library/Logs/...`,
`~/.local/share/RarZap/logs`). Logs prune to 7 days; on Windows the release
build is a GUI binary, so there is no console layer.

## 6. Build & packaging

### 6.1 Prerequisites

| Platform | Build deps | Target |
|---|---|---|
| macOS | Xcode.app (Metal shader compilation) | `aarch64-apple-darwin` |
| Windows | Visual Studio Build Tools | `x86_64-pc-windows-msvc` |
| Ubuntu/Debian | GTK3/Wayland/Vulkan dev packages, libxcb, libxkbcommon | `x86_64-unknown-linux-gnu` |

Ubuntu 24.04 packages are listed verbatim in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

### 6.2 Commands

```bash
cargo build --release --locked
cargo test --locked
```

`--locked` pins to the committed `Cargo.lock`.

### 6.3 Packages

| Platform | Package |
|---|---|
| Windows | Inno Setup installer + uninstaller (`packaging/windows/rarzap.iss`) |
| macOS | Signed + notarized `.dmg` (`packaging/macos/`) |
| Debian/Ubuntu | `.deb` (`packaging/linux/`) |

Icons for all three are generated from one hand-drawn source,
`assets/logo-rar.png`, by `cargo run --example generate_icons` — no external
toolchain, fully reproducible.

## 7. CI

GitHub Actions, **manually triggered** (`workflow_dispatch`). One matrix job per
platform builds once (`cargo build --release`), reuses that build for
`cargo test --release`, then packages and uploads artifacts. A separate
`audit` job runs the RustSec advisory check against `Cargo.lock`. Third-party
actions are pinned to commit SHAs. A tagged release is the only path that
publishes a GitHub Release.

## 8. Repository layout

```
src/
  core/       single-file RAR→ZIP primitive + RarzapPlugin trait
  free/       free feature: single-file plan/run service
  ui/         free UI: Empty / Working / Done, Settings, Privacy
  i18n.rs     8-locale setup (rust-i18n, compiled-in YAML)
  logging.rs  tracing init, rotation, pruning
assets/       icons (generated), logo source, screenshots
packaging/    Inno Setup (.iss) · macOS (.plist + make-app.sh) · Debian (.deb)
plugins/      commercial batch plugin — LOCAL ONLY, git-ignored
docs/         this document + UI design
```

## 9. Risks & mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| GPUI pre-1.0 churn | Upgrades may need code changes | Pin `Cargo.lock`; upgrade deliberately |
| Linux desktop variance | Dialog/decoration differences | XDG Portal; test on GNOME + KDE |
| Large archive memory | 10 GB+ RAR could OOM | Streaming extract→pack; never load whole archive (library-level streaming is not exposed by `unrar-ng`, so files go through a staging directory) |
| Filename encoding | CJK names garbled | Handled by `unrar-ng` / lossless wide strings |
| macOS notarization | Setup friction | `notarytool` in CI, gated on secrets |
