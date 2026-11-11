# Development

This guide is for building, testing, packaging, and changing VibranceGUI for Linux. For user-facing behavior, start with the [user guide](USAGE.md); for the config model, see the [configuration reference](CONFIGURATION.md).

## Build

### Toolchain

- Rust **1.95 or newer** (the crate uses edition 2024).
- Cargo and the usual linker/build tools.
- Native development packages for GL/EGL, X11, Wayland, fontconfig, OpenSSL, and `pkg-config`.

A stable toolchain is sufficient:

```bash
rustup toolchain install stable
rustup default stable
```

### Debian / Ubuntu

The CI and release images use Ubuntu 24.04. Install:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential pkg-config libssl-dev \
  libgl1-mesa-dev libegl1-mesa-dev libxkbcommon-dev libwayland-dev \
  libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxinerama-dev \
  libfontconfig1-dev
```

### Arch Linux

```bash
sudo pacman -S --needed base-devel rust pkg-config openssl \
  libglvnd wayland libxkbcommon libx11 libxcursor libxi libxrandr libxinerama fontconfig
```

The AUR source package declares the runtime libraries as `depends`, builds with `cargo build --release --locked`, and assumes `base-devel` plus `rust`.

### Compile and run

```bash
cargo build --release --locked
./target/release/vibrancegui
```

The binary is `target/release/vibrancegui`. A debug build is also supported:

```bash
cargo run -- --no-tray
```

If the GUI cannot open in a headless environment, `--doctor` still works without a window:

```bash
cargo run -- --doctor
```

## Checks

Run the same checks as CI before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
```

The unit tests do not modify real Steam, vkSumi, or profile files. Tests that need a writable directory use `tempfile`. The suite covers VDF parsing, launch-option token matching, profile line editing, atomic save/backup behavior, process-name normalization, and state defaults.

There is no integration test that launches a Vulkan game. Validate layer detection and live edits manually on a real Vulkan title.

## Project structure

```text
Cargo.toml                 package metadata, dependencies, release profile
Cargo.lock                 locked dependency graph
src/
  main.rs                  argument handling, window setup, --doctor, tray-only helper
  app.rs                   egui UI, game grid, profile editor, state transitions
  steam.rs                 Steam roots, libraries, manifests, launch options, artwork lookup
  vdf.rs                   read-only Valve KeyValues parser
  shortcuts.rs             binary shortcuts.vdf parser
  process.rs               /proc environment/cmdline/maps observation
  config_doc.rs            line-preserving vkSumi .conf editor and control definitions
  artwork.rs               Steam CDN artwork, .exe icon extraction, caches
  state.rs                 tracked games, manual games, remembered paths, theme
  paths.rs                 XDG paths and path expansion
  theme.rs                 dark/light palettes and system theme detection
  tray.rs                  StatusNotifierItem tray and tray-only helper
  vksumi.rs                layer manifest/library validation
packaging/
  aur/PKGBUILD.in          AUR source package template
  aur/PKGBUILD-bin.in      AUR prebuilt package template
  vibrancegui.desktop      desktop entry
  vibrancegui.svg/.png     application icon
.github/workflows/
  ci.yml                   formatting, clippy, tests, release build
  release.yml              tagged binaries, AppImage, checksums, AUR updates
docs/                      user and contributor documentation
```

There is no library crate or public API; the package produces one binary.

## Architecture

### Steam discovery (`steam.rs`, `vdf.rs`, `shortcuts.rs`)

`SteamData::discover()`:

1. finds native Steam roots (`~/.steam/steam`, `~/.local/share/Steam`, `$XDG_DATA_HOME/Steam`);
2. reads `steamapps/libraryfolders.vdf` and app manifests;
3. reads the newest `userdata/<account>/config/localconfig.vdf` for launch options; if that file cannot be read or parsed, launch options are unavailable;
4. reads `shortcuts.vdf` for Steam shortcuts;
5. prefers local artwork, otherwise leaves the artwork worker to fetch it.

All Steam reads are read-only. The VDF parser intentionally supports only what the app needs.

### Process observation (`process.rs`)

The observation thread scans `/proc` about every two seconds. It reads:

- `environ` for `ENABLE_VKSUMI`, `DISABLE_VKSUMI`, `SteamAppId`/`SteamGameId`, and `VKSUMI_CONFIG_FILE`;
- `cmdline` and `exe` for the executable name/path;
- `maps` for `libVkLayer_vksumi.so`.

The result is an `Observation` sent to the UI. No other environment variables are retained. The scanner stops when the app/helper thread exits.

### Profile editing (`config_doc.rs`, `app.rs`)

`ConfigDoc` keeps the original lines and edits only active `key = value` lines. Saves are debounced and use a temporary file plus rename. `state.rs` remembers observed profile paths so they remain editable after the game exits.

Profile resolution in `app.rs` is described in the [configuration reference](CONFIGURATION.md#profile-resolution). The effective path is deliberately conservative: a path outside the vkSumi config directory is opened read-only.

### UI and helpers (`app.rs`, `theme.rs`, `tray.rs`, `artwork.rs`)

- `eframe`/`egui` with the `glow` renderer; X11 and Wayland features are enabled.
- `dark-light` detects the system theme; the saved choice is applied at startup.
- `ksni` provides the StatusNotifierItem tray. Wayland close-to-tray delegates to a separate `--tray-only` process.
- `rfd` uses the desktop portal for file pickers.
- `ureq` downloads Steam artwork from `cdn.cloudflare.steamstatic.com`; `icoextract_rs` extracts icons from Windows PE files.

### Layer validation (`vksumi.rs`)

The app checks host and common Flatpak/manual manifest locations. A manifest is usable only when:

- it contains a vkSumi layer name;
- its `library_path` points to an existing library;
- its `enable_environment` requires `ENABLE_VKSUMI=1`.

A library found without a readable manifest is treated as usable with a warning; the running game remains the source of truth.

## Tests and manual validation

Automated:

```bash
cargo test --all-targets
```

Manual checklist for UI or detection changes:

1. `--doctor` finds the expected Steam root and games.
2. A game with `ENABLE_VKSUMI=1` appears without **Show all**.
3. Launching the game creates or finds a profile and the detail view reports the layer state.
4. Moving a slider writes the value and creates a backup.
5. **reset** removes the key; **Reset overrides** removes all editor-managed keys.
6. External paths show the read-only warning and cannot be edited.
7. Close-to-tray works on X11 and Wayland, or falls back cleanly when no tray host exists.

Use a disposable `HOME`/XDG directory when testing UI changes that should not touch your real library:

```bash
mkdir -p /tmp/vibrancegui-test
HOME=/tmp/vibrancegui-test \
XDG_CONFIG_HOME=/tmp/vibrancegui-test/.config \
XDG_CACHE_HOME=/tmp/vibrancegui-test/.cache \
XDG_DATA_HOME=/tmp/vibrancegui-test/.local/share \
./target/release/vibrancegui --no-tray
```

## Release workflow

A tag matching `v*` starts `.github/workflows/release.yml`. It can also be run manually with an existing tag. The workflow:

1. checks formatting and clippy;
2. runs the tests;
3. builds `target/release/vibrancegui` with `cargo build --release --locked`;
4. builds `vibrancegui-linux-<version>-x86_64.AppImage` with `appimagetool`;
5. publishes the native binary, AppImage, and `SHA256SUMS` to the GitHub release;
6. renders `packaging/aur/PKGBUILD.in` and `PKGBUILD-bin.in`;
7. publishes both `vibrancegui-linux` and `vibrancegui-linux-bin` to the AUR.

Required repository secrets:

- `AUR_USERNAME`
- `AUR_EMAIL`
- `AUR_SSH_PRIVATE_KEY`

The AUR `-bin` package uses the release binary and the tag archive (for docs and license files); the source package builds locally. They conflict with each other and provide the same `vibrancegui` executable.

The AppImage uses host GL/EGL, Wayland/X11, OpenSSL, and font libraries plus the desktop portal. It does not bundle a Vulkan driver, vkSumi, or a portal backend. Flatpak is intentionally not shipped yet because host Steam config, profile paths, and layer visibility need a separate design.

## Roadmap

These are planned, not implemented in the current 0.2.x line:

- **Managed Steam launch options.** Write and roll back `ENABLE_VKSUMI=1` safely, with Steam-closed enforcement, backups, and drift detection.
- **Stable per-AppID profiles.** Use `VKSUMI_CONFIG_FILE` consistently so games that share an executable do not share a profile by accident.
- **Censor-box editor and daltonize-mode selector.** The numeric daltonize controls exist; the censor-box editor and daltonize-mode selector remain config-file-only.
- **Launcher imports.** Heroic/Lutris import after their config formats are verified.
- **Flatpak.** Only if a safe host-config, profile-path, and Vulkan-layer integration can be designed.

## Documentation

- [README](../README.md) — project entry point and quick start
- [User guide](USAGE.md) — UI and workflows
- [Configuration](CONFIGURATION.md) — paths, precedence, controls, file format
- [Troubleshooting](TROUBLESHOOTING.md) — common failures
- [Contributing](../CONTRIBUTING.md) — issue and PR process
- [Security](../SECURITY.md) — data handling and reporting

When changing a CLI flag, config path, UI control, package name, or release process, update the corresponding document and the command examples in the same pull request.
