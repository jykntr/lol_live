# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                  # Debug build
cargo build --release        # Release build
cargo check                  # Quick type/syntax check
cargo test                   # Run tests
cargo clippy --all-features  # Lint
cargo fmt                    # Format
```

Formatting is enforced automatically via a hook that runs `cargo fmt` on file edits.

### System Dependencies

- **macOS:** `brew install tesseract leptonica`
- **Windows:** vcpkg with `tesseract` and `leptonica` (see `vcpkg.json`)
- **Linux:** `apt install libleptonica-dev libtesseract-dev clang pkg-config` + X11/Wayland libs

## CI/Build Considerations

- GitHub Actions may build on Linux for speed, so ensure code is cross-compile friendly
- Avoid platform-specific assumptions in build scripts

## Platform Support

- **Target platforms**: Windows and macOS (Apple Silicon primary, Intel optional)
- **Development/CI**: Builds run in GitHub Actions, may use Linux for faster compilation

## Architecture

Real-time League of Legends CS (creep score) tracker that combines two data sources: the in-game API and OCR screen
capture.

### Runtime Model

Three concurrent loops share state via `tokio::sync::watch` channels:

```
API loop (3s, async)  ──→ watch<Option<ApiData>> ──→ Display loop (500ms, async)
                              ↓                           ↑
OCR loop (1s, spawn_blocking) ──→ watch<Option<i64>> ─────┘
```

- **API loop** polls `https://127.0.0.1:2999/liveclientdata/allgamedata` (League's local HTTPS endpoint, requires
  `danger_accept_invalid_certs`)
- **OCR loop** waits for the API to detect a running game, then captures the primary monitor, crops the CS region, and
  runs Tesseract. Pauses when the game ends.
- **Display loop** merges both sources, preferring OCR CS when available, and prints CS/min stats.

OCR runs in `spawn_blocking` because Tesseract is CPU-bound and not async-safe.

### Module Responsibilities

| Module       | Role                                                                                        |
|--------------|---------------------------------------------------------------------------------------------|
| `main.rs`    | Spawns all three loops, wires channels, loads config                                        |
| `api.rs`     | Single function `poll_game_data` → `Option<ApiData>`                                        |
| `config.rs`  | Parses League's `game.cfg` INI file for HUD scale and resolution (platform-specific paths)  |
| `capture.rs` | Screen capture via `xcap`, cropping, grayscale conversion                                   |
| `ocr.rs`     | Tesseract init (embeds traineddata), preprocessing (threshold + upscale), OCR execution     |
| `detect.rs`  | Calculates CS counter screen region based on resolution and HUD scale (baseline: 1920x1080) |
| `types.rs`   | `CsRegion` and `ApiData` structs                                                            |

### Key Design Decisions

- **Graceful degradation:** If Tesseract can't initialize, runs in API-only mode.
- **Embedded traineddata:** `eng.traineddata` is compiled into the binary via `include_bytes!` and extracted to a temp
  dir at runtime.
- **HUD scale:** Read from League's `game.cfg` (`[HUD] GlobalScale`). The CS region coordinates scale proportionally
  from a 1920x1080 baseline.
- **Debug mode:** Pass `--debug` to enable screenshot saves and OCR diagnostic output to stderr.

### CI

GitHub Actions in `.github/workflows/`:

- `build.yml` — runs check, test, fmt, clippy on push; matrix builds for Windows x64 + macOS ARM64
- `release.yml` — builds and uploads binaries on GitHub release creation
