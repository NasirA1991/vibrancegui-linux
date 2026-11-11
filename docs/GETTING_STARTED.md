# Getting started

This is the shortest path from a fresh install to a working vkSumi profile. It assumes a native Steam game. For the full UI reference, see the [user guide](USAGE.md); for paths and every control, see [Configuration](CONFIGURATION.md).

## What you need

To use the application itself:

- Linux with Wayland or X11.
- A native Steam installation, if you want automatic game discovery.

To actually change a game's colours, you also need:

- [vkSumi](https://github.com/reakjra/vkSumi) installed and visible to the game;
- a Vulkan game;
- `ENABLE_VKSUMI=1` in that game's Steam launch options.

**VibranceGUI for Linux is a profile manager, not the Vulkan layer.** It can start, discover games and edit profiles without vkSumi, but the game will not be graded until vkSumi is installed and loaded. The app never installs vkSumi, invokes a package manager, or asks for root access.

## 1. Install VibranceGUI for Linux

On Arch Linux or CachyOS, choose one AUR package:

```bash
paru -S vibrancegui-linux-bin      # prebuilt binary
# or
paru -S vibrancegui-linux          # builds from source
```

Both provide the same `vibrancegui` command and conflict with each other.

Alternatively, download the x86_64 AppImage or native binary from the [latest release](https://github.com/NasirA1991/vibrancegui-linux/releases/latest), or [build from source](DEVELOPMENT.md#build). When using the AppImage, replace `vibrancegui` with `./vibrancegui-linux-*.AppImage` in the commands below.

## 2. Install vkSumi

Install vkSumi separately and make sure the game can see the layer. On Arch Linux and CachyOS:

```bash
paru -S vksumi
```

On other distributions, follow the [vkSumi installation instructions](https://github.com/reakjra/vkSumi). If vkSumi is already installed, reuse that installation; do not add a second copy of the layer.

Check what the app can see:

```bash
vibrancegui --doctor
```

The header should say **vkSumi detected** after a refresh. If it does not, see [vkSumi is not detected](TROUBLESHOOTING.md#vksumi-is-not-detected).

## 3. Enable vkSumi for the game

In Steam:

1. Right-click the game and choose **Properties**.
2. Open **Launch Options**.
3. Add the variable before `%command%`:

   ```text
   ENABLE_VKSUMI=1 %command%
   ```

If the game already uses wrappers, keep the variable at the front:

```text
ENABLE_VKSUMI=1 mangohud gamemoderun %command%
```

If the existing launch options do not use `%command%`, preserve their structure and make sure `ENABLE_VKSUMI=1` appears as a separate environment assignment. The game detail window offers **Copy launch option** when the variable is missing.

vkSumi is an implicit Vulkan layer. It must be enabled before the game creates its Vulkan instance, so the app cannot activate it inside an already-running game. The app also does not edit Steam launch options or Steam's VDF files. This step is manual by design.

To disable vkSumi for one game later, remove `ENABLE_VKSUMI=1` or add `DISABLE_VKSUMI=1`.

## 4. Start VibranceGUI for Linux

Launch `vibrancegui` from your application menu or a terminal:

```bash
vibrancegui
```

Useful first-run options:

| Option | Effect |
| --- | --- |
| `--doctor` | Print Steam, vkSumi and launch-option diagnostics without opening the window. |
| `--doctor --observe` | Also list currently running vkSumi processes and their config paths. |
| `--no-tray` | Disable the tray and use normal close behavior. |
| `--hidden` | Start hidden if a tray host is available. |

The main window has four areas:

- **Header:** vkSumi status, **Refresh**, and the **☰** menu.
- **Settings:** autostart, the global **Default Vibrance Level**, and a collapsible **Configuration** section.
- **Program Settings:** the game grid, search, filtering and add/remove actions.
- **Footer:** vkSumi status, tracked-game count and the installed version.

The **☰** menu contains **Theme**, **About VibranceGUI** and **Quit**. The theme choice is saved. The tray, if available, adds **Show**, **Refresh Steam library** and **Quit**.

## 5. Launch the game and wait for detection

Start the game from Steam while the app is running. The app scans `/proc` roughly every two seconds. Within a few seconds the game tile should show one of these states:

- **Detected · vkSumi layer mapped** — the process is running and `libVkLayer_vksumi.so` is in its memory maps.
- **Detected · layer unconfirmed** — the process has `ENABLE_VKSUMI=1`, but the layer is not visible in its maps or the maps could not be read.

For reliable profile linking, start the app before the game. Opening the app after the game starts can still find the process, but the first scan is the most reliable point to capture the effective config path.

If the game tile is missing, use **Show all** to browse the full installed library. **Add** can track an installed game regardless of its launch options, but it does not enable vkSumi.

## 6. Open the profile and adjust it

Select the game tile. The settings window shows:

- the game name and Steam AppID;
- the launch-option or shortcut status;
- the effective vkSumi profile path;
- **Basic** controls, starting with **Vibrance**;
- **Advanced (all vkSumi sliders)** with the other 26 numeric controls;
- **Reset overrides** and **Open config**.

Every control can be dragged or typed. A **?** explains the control, and **reset** removes that key so the game inherits the value from the global profile. **Reset overrides** removes every editor-managed key from this game profile; unrelated keys such as `enabled`, censor settings and daltonize mode are left alone.

Changes are written to the profile and vkSumi hot-reloads them. The app does not inject into the game or draw an overlay. If colours do not change, confirm that the layer is mapped and that the selected profile is the one the game opened.

### Global versus per-game values

- The main window's **Default Vibrance Level** writes only `vibrance` to the global `vkSumi.conf`.
- Per-game sliders write only the values you change to that game's profile.
- Values not overridden are inherited from the global profile.
- Negative vibrance is not exposed by the UI; it remains a config-file-only value.

A profile outside the vkSumi configuration directory opens read-only. This protects configs managed by GOverlay or another tool. See [External profiles](CONFIGURATION.md#external-profiles) if you need to move one under the managed directory.

## 7. Optional: add a non-Steam game

Use **Add manually** for a game that is not launched through native Steam and was not auto-detected while running. You need the game executable, optionally an icon, and optionally an absolute vkSumi config path.

Launch the game with `ENABLE_VKSUMI=1` from its own launcher. The app matches the process by executable name. If it was started without the variable, it will not be adopted. See [Adding games](USAGE.md#adding-games) for details.

## If something does not work

| Problem | Check |
| --- | --- |
| Header says **vkSumi not detected** | vkSumi installation, layer architecture, `vibrancegui --doctor`, and the game's launch options. |
| Game is missing from the grid | **Show all**, the exact `ENABLE_VKSUMI=1` token, and **Refresh**. |
| Game is running but layer is unconfirmed | `DISABLE_VKSUMI=1`, anti-cheat, 32/64-bit layer mismatch, and the game's Vulkan renderer. |
| Controls are read-only | The profile is outside the vkSumi config directory. |
| Close button does not hide the window | A StatusNotifierItem host is required; use `--no-tray` for normal close behavior. |

See [Troubleshooting](TROUBLESHOOTING.md) for the full list, or open an [issue](https://github.com/NasirA1991/vibrancegui-linux/issues) with the details requested in [Contributing](../CONTRIBUTING.md#reporting-a-bug).

## Next steps

- [User guide](USAGE.md) — game management, discovery states, tray behavior and CLI.
- [Configuration](CONFIGURATION.md) — profile resolution, all controls and file locations.
- [Troubleshooting](TROUBLESHOOTING.md) — layer, launch and packaging failures.
