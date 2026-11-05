# VibranceGUI for Linux

**Unofficial.** A lightweight, native Linux companion for [vkSumi](https://github.com/reakjra/vkSumi), inspired by the Windows [VibranceGUI](https://vibrancegui.com/). Not affiliated with the Windows project.

- Source: <https://github.com/NasirA1991/vibrancegui-linux>
- AUR: <https://aur.archlinux.org/packages/vibrancegui-linux>
- Release: <https://github.com/NasirA1991/vibrancegui-linux/releases/tag/v0.1.0>

VibranceGUI for Linux v0.1 is a **vkSumi profile editor with Steam discovery and running-game detection**. It does **not** write Steam launch options yet. You add the vkSumi environment variable once; the app finds the game, links the right config, and lets you tune it live.

## What works

- Scans installed Steam games, libraries, names and AppIDs.
- Reads each game's Steam launch options and shows whether `ENABLE_VKSUMI=1` is present.
- Detects currently running games from `/proc` when they carry `ENABLE_VKSUMI=1`.
- Checks `/proc/<pid>/maps` for `libVkLayer_vksumi.so`, so “running” is not confused with “vkSumi loaded”.
- Finds the effective profile path:
  - `VKSUMI_CONFIG_FILE` from the running process, if present;
  - otherwise vkSumi's per-executable `~/.config/vkSumi/games/<exe>.conf`.
- Edits the profile live; vkSumi hot-reloads it.
- A global default vibrance slider that writes only `vibrance` to the global vkSumi config.
- Basic per-game controls (Vibrance, Saturation, Temperature, Contrast, Brightness, Gamma, Exposure).
- Atomic profile writes and timestamped backups.
- Never writes Steam configuration in this version.
- Runs entirely on demand: process scanning stops when the window closes. No daemon, tray, or autostart.

## What does not work yet

- No automatic Steam launch-option editing.
- No Enable/Disable button. Removing `ENABLE_VKSUMI=1` from Steam is still the only true off switch.
- No Heroic, Lutris, Faugus, or Flatpak Steam support.
- No censor-box editor or full advanced filter set.
- HDR/non-sRGB games may show no effect; OpenGL games do not work; anti-cheat may block Vulkan layers.

## Requirements

- Linux, Wayland or X11.
- A Vulkan-capable game.
- [vkSumi](https://github.com/reakjra/vkSumi) installed (AUR, Flatpak or manual build).
- Rust 1.95+ and the usual native build dependencies for `eframe`/`egui` (X11/Wayland/OpenGL headers).

## Install

### AUR (Arch / CachyOS)

```bash
paru -S vibrancegui-linux
# or
yay -S vibrancegui-linux
```

If your AUR helper does not see the new package yet, refresh its databases first (`paru -Sy`).

### Build from source

```bash
cargo build --release
```

The binary is `target/release/vibrancegui`.

Run:

```bash
./target/release/vibrancegui
```

A prebuilt binary is available on the [GitHub release page](https://github.com/NasirA1991/vibrancegui-linux/releases/tag/v0.1.0).

## First-time use

1. In Steam, right-click the game → **Properties → Launch Options**.
2. Add the vkSumi environment variable. If you already have wrappers, put it at the front:

   ```text
   ENABLE_VKSUMI=1 %command%
   ```

   or, with existing options:

   ```text
   ENABLE_VKSUMI=1 mangohud gamemoderun %command%
   ```

3. Start VibranceGUI for Linux and keep it open.
4. Launch the game. Within a few seconds the game tile should show **running**.
5. Select the game and tune the sliders.

Once a game has been detected, the app remembers the config path, so you can edit it later even when the game is not running.

The app never modifies Steam's files; if the launch option is missing, it shows a copyable line and asks you to add it once.

## Why the launch option is still manual

vkSumi is a Vulkan layer. It must be present before the game creates its Vulkan instance, so it cannot be enabled inside an already-running game. The v0.1 scope deliberately avoids Steam VDF editing; automatic launch-option management is the next milestone.

## Global default vs per-game profiles

- The **Default** slider writes `vibrance` to `~/.config/vkSumi/vkSumi.conf`.
- A per-game slider writes an override to that game's profile.
- **reset** on a control removes the override, so the game inherits the global value.
- **Reset overrides** removes all app-managed overrides from that profile.

Only the seven basic controls are exposed in this version; the profile file remains a normal vkSumi config and can still contain advanced keys.

## Safety

- Steam files are **read-only** in v0.1.
- Only vkSumi config files are written.
- Writes use a temporary file + rename and a timestamped backup under `~/.config/vibrancegui/backups`.
- Config paths outside `~/.config/vkSumi` are shown read-only, so GOverlay or custom configs are not silently changed.
- No root, no package-manager invocation, no daemon.
- `/proc` is scanned only while the window is open. Only `ENABLE_VKSUMI`, `DISABLE_VKSUMI`, `SteamAppId`, `SteamGameId`, and `VKSUMI_CONFIG_FILE` are used; other environment variables are not logged or stored.

## Diagnostics

```bash
vibrancegui --doctor
vibrancegui --doctor --observe
```

If running from the build tree, use `target/release/vibrancegui --doctor`.

`--doctor` prints the vkSumi manifest/library status, Steam root, account, installed games and their launch-option state. `--observe` also prints currently detected vkSumi-enabled processes, their AppIDs, executable names, layer-map state and effective config path.

## Project layout

```text
src/
  main.rs        app bootstrap, theme, doctor
  app.rs         egui UI, game grid, profile editor
  steam.rs       Steam roots, libraries, manifests, launch options, artwork
  vdf.rs         read-only Valve KeyValues parser
  process.rs     /proc environment/cmdline/maps observation
  config_doc.rs  line-preserving vkSumi .conf editor
  vksumi.rs      layer manifest health check
  state.rs       remembered AppID -> profile mapping
  paths.rs       XDG paths and path expansion
packaging/
  vibrancegui.desktop
  vibrancegui.svg
```

## Tests

```bash
cargo test
```

The test suite covers VDF parsing, launch-option token handling, profile line editing, atomic save/backup behavior, and process-name normalization. Automated tests never touch real Steam files.

## Roadmap

- **v0.2:** opt-in managed Steam launch-option writing, stable AppID profiles via `VKSUMI_CONFIG_FILE`, backups/rollback, drift detection.
- Censor-box editor.
- Full advanced filters.
- More launchers only after their config formats are verified.

## License and naming

MIT.

This is an unofficial community project. “VibranceGUI” is used descriptively to describe the interaction model; it is not affiliated with, endorsed by, or sponsored by the Windows VibranceGUI project. vkSumi is a separate MIT project and is not bundled.
