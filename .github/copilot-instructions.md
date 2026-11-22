# Repository instructions

## Build, test, and lint

Use a stable Rust toolchain (`edition = "2021"`). On Debian/Ubuntu, install the desktop build dependencies with:

```sh
sudo apt-get install build-essential pkg-config libgl1-mesa-dev xorg-dev
```

Run the app in a graphical desktop session with `cargo run`, or build a release binary with `cargo build --release`.

Run all headless unit and UI integration tests (no display required) with:

```sh
cargo test --locked
```

Run one test by name with:

```sh
cargo test --locked test_todo_lifecycle
```

Run strict linting with:

```sh
cargo clippy --all-targets --locked -- -D warnings
```

## Architecture

This is a Rust desktop application built on `eframe` / `egui` (`0.31`) and `rusqlite` (bundled SQLite).

- `src/main.rs` configures the viewport (`760x600`, embedded `assets/icon.ico`), opens the database under the user's config directory (`scrivi/todos.sqlite`), applies the persisted theme, starts the Windows update watcher on release builds, and launches `eframe`.
- `src/store.rs` implements `MemoryDb`: in-memory maps hold the UI-facing view, while SQLite persists each change immediately and supplies data at startup. A mutation reaches disk (often transactionally) before the in-memory maps change, so failed writes preserve the prior view. Run only one app instance at a time.
- `src/theme.rs` defines all five app themes (`Pastel`, `Ocean`, `Sage`, `Rose`, `Light`), category colors, WCAG contrast verification, and `egui` visual styling.
- `src/icons.rs` maps the 48 `CATEGORY_EMOJIS` directly to `twemoji-assets` PNG constants (without calling `from_emoji()`) and renders vector UI icons.
- `src/layout.rs` computes responsive category grid sizing, centered truncating category headings, and drag-and-drop insertion indices.
- `src/ui.rs` implements `ScriviApp` (`eframe::App`), exposing `ScriviApp::render(&mut self, &egui::Context)` for headless UI testing, nested edge-propagating scrolling, drag reordering, `Shift+Enter` multiline input, and stacked modals.
- `src/options.rs` handles Windows always-on-top pinning (`SetWindowPos`) and sign-in startup registration (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) via a testable `WindowPlatform` trait.
- `src/updates.rs` checks GitHub releases at startup and every five minutes on Windows release builds (`SCRIVI_VERSION` set at compile time; `dev` skips checks). It validates stable `vMAJOR.MINOR.PATCH` versions, builds a fixed GitHub release download URL, verifies the `.sha256` checksum, launches Inno Setup (`/SILENT /NORESTART /SP- /RESTARTSCRIVI=1`), and quits.
- `build.rs` embeds `assets/icon.ico` into Windows builds via `winresource`, and `.cargo/config.toml` enables `+crt-static` on `x86_64-pc-windows-msvc` so release executables require no external VC++ runtime DLLs.

## Repository conventions

- Keep storage changes durable before updating cached state. Return contextual errors to callers; the UI presents operation errors in modal dialogs.
- Storage, theme, options, icon, update, and full headless `egui` UI integration tests (`tests/ui_tests.rs`) must run and pass without a graphical display.
- Windows CI runs `cargo test --locked`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo build --release --locked` on `windows-2022` (MSVC).
- Successful pushes to `main` automatically increment the release version and publish an unsigned Windows installer, SHA-256 checksum, and GitHub Release. Explicit stable `vMAJOR.MINOR.PATCH` tags can choose a version instead. Version components must be at most 65535, and published versions must increase; do not move or reuse release tags.
- The Inno Setup `AppId` in `packaging/windows/scrivi.iss` must remain unchanged so upgrades target the existing installation. Installer/uninstaller operations must leave the SQLite database in the user's AppData directory untouched.

Commit and push to GitHub after every fix/feature.
