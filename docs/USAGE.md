# Using VibranceGUI for Linux

This guide covers the desktop UI, game discovery, launch options, profile editing, tray behavior, files, and diagnostics. For every config key and path rule, see the [configuration reference](CONFIGURATION.md). For fixes, see [Troubleshooting](TROUBLESHOOTING.md).

## What the app does

VibranceGUI for Linux manages [vkSumi](https://github.com/reakjra/vkSumi) color-grading profiles. It can discover games, associate a game with its vkSumi config, and edit that config while the game is running.

It does **not**:

- install vkSumi or a Vulkan driver;
- inject into a game or draw an overlay;
- modify Steam launch options or other Steam files;
- apply a grade itself. vkSumi does that when the game creates its Vulkan device.

Because of that split, the `ENABLE_VKSUMI=1` launch option is still a one-time manual step in Steam. The app reads the option to decide whether to show the game and whether editing is available.

## Before you start

You need:

- a Vulkan game;
- vkSumi installed and visible to the game;
- the game launched with `ENABLE_VKSUMI=1`;
- VibranceGUI for Linux running while you launch the game (for automatic detection and profile linking).

If vkSumi is missing, the header shows **vkSumi not detected** and the app displays an install hint. The app can still edit a profile path that is already known, but the game will not be graded until the layer is available. See [Installing vkSumi](TROUBLESHOOTING.md#vksumi-is-not-detected).

## Main window

The main window has four areas:

- **Header.** Shows whether vkSumi was detected, a **Refresh** button, and the **☰** menu.
- **Settings.** Contains the default vibrance level, the autostart checkbox, and a collapsed **Configuration** section showing the global vkSumi config path.
- **Program Settings.** The game grid, search field, **Show all**, **Add**, **Add manually**, and **Remove** controls.
- **Footer.** Shows the current status and app version.

The **☰** menu contains:

- **Theme → System default / Dark / Light**
- **About VibranceGUI** with the version, repository, and issue links
- **Quit**

The selected theme is saved. The window can be resized; game settings open in a separate native window that can be moved independently of the main window.

## Adding games

### Games that are already enabled

On first run, the app tracks Steam games whose launch options contain `ENABLE_VKSUMI=1`, plus installed Steam shortcuts. A normal Steam game is shown when at least one of these is true:

- its launch options contain `ENABLE_VKSUMI=1`;
- it is currently running with vkSumi enabled.

Use **Show all** to browse the rest of the installed Steam library. Selecting **Only enabled** returns to the filtered view. The **?** next to **Program Settings** repeats the rule in the UI. **Add** can track any installed game regardless of these conditions; it does not add the launch option.

### Add a Steam game

1. Click **Add**.
2. Search for an installed Steam game.
3. Click **+ Game name**.

The game is added to the tracked list and its settings open immediately. Because the filtered grid only shows enabled or running games, a game added without `ENABLE_VKSUMI=1` may disappear from the grid after you close its settings; use **Show all** to find it again. Adding it does not add the launch option; enable vkSumi in Steam before the layer can load.

### Add a non-Steam game

Use **Add manually** when a game:

- is not launched through native Steam, or
- has no Steam shortcut, and
- has not been auto-detected while running.

Fill in:

- **Executable** — the game binary or a Windows `.exe` inside a Proton prefix. The file must exist.
- **Display name** — optional; defaults to the executable name.
- **Icon path** — optional image; Windows `.exe` icons can also be extracted automatically.
- **vkSumi config** — optional absolute path. If left empty, the app uses `games/<executable-stem>.conf` under the vkSumi config directory.

Click **Add game**. Launch the executable with `ENABLE_VKSUMI=1` from its launcher. The app matches the running process by executable name and, when possible, records the effective `VKSUMI_CONFIG_FILE` from that process.

### Automatic detection of non-Steam games

While the app is running, a process that:

- has `ENABLE_VKSUMI=1` in its environment,
- does not have `DISABLE_VKSUMI=1`,
- is not identified as a Steam game, and
- has a readable executable path

is adopted into the manual-game list automatically. This is useful for Heroic, Lutris, Bottles, Faugus, and other launchers when their process environment is visible to the app. It is not a library import: the game appears only after it has run.

### Remove a game

Select a tile and click **Remove**. This removes the app's tracking entry only. It does not delete the vkSumi profile, Steam data, or the executable.

## Steam launch options

Open the game in Steam, choose **Properties → Launch Options**, and add the variable before `%command%`:

```text
ENABLE_VKSUMI=1 %command%
```

With existing wrappers, keep the variable at the front:

```text
ENABLE_VKSUMI=1 mangohud gamemoderun %command%
```

Some games have launch options without `%command%` (for example, a URL or a custom argument). The app's suggested line handles that case, but the important part is that `ENABLE_VKSUMI=1` is present as a separate token in the final environment.

To turn vkSumi off for one game:

- remove `ENABLE_VKSUMI=1`, or
- add `DISABLE_VKSUMI=1`. vkSumi treats an explicit disable as authoritative.

The app never changes these options. The detail view offers **Copy launch option** when the variable is missing.

vkSumi is an implicit Vulkan layer, so it must be enabled before the game creates its Vulkan instance. The app cannot turn it on inside an already-running game. It also does not write Steam's files; automatic launch-option management is planned but not implemented in 0.2.x.

## Detecting a running game

The app scans `/proc` while it is running, approximately every two seconds. For each candidate it checks:

- the `ENABLE_VKSUMI` and `DISABLE_VKSUMI` environment variables;
- `SteamAppId` / `SteamGameId`, when present;
- the process's executable name and path;
- `/proc/<pid>/maps` for `libVkLayer_vksumi.so`;
- `VKSUMI_CONFIG_FILE` when present.

A detected game is linked to the profile path reported by the process, or to the per-executable path when the process has no explicit config variable. The detail view distinguishes:

- **Detected · vkSumi layer mapped** — the process is running and the layer library is mapped.
- **Detected · layer unconfirmed** — the process has the environment variable, but the layer is not visible in its memory maps or the maps could not be read.

The second state can still be useful for editing a known config, but it usually means the layer did not load for that process. See [the game runs but colors do not change](TROUBLESHOOTING.md#the-game-runs-but-colors-do-not-change).

You can open the app after the game has started; the first scan can still find it. For reliable profile linking, start the app before launching the game.

## Editing a profile

Select a game tile to open its settings window.

### Basic and Advanced

- **Basic** shows **Vibrance**, the main control.
- **Advanced (all vkSumi sliders)** contains the remaining 26 numeric controls.

Every control supports:

- dragging the slider;
- typing a value in the number field;
- **reset**, which removes that key from the profile so the global value applies;
- a **?** tooltip explaining the effect.

**Reset overrides** at the bottom of the window removes every control managed by the editor from this profile. It does not remove unrelated vkSumi keys such as `enabled`, daltonize mode, or censor settings.

### Global values and per-game overrides

The **Default Vibrance Level** in the main window is stored in the global vkSumi config. A per-game profile stores only the values you explicitly change, and inherits the rest from the global profile.

- A percentage in the main window is a normalized `vibrance` value: 50% writes `0.500`.
- A value shown in muted text in a game's settings is inherited from the global profile.
- A value shown in normal text is an override stored in the game profile.
- Negative vibrance is not exposed by the UI; it remains a config-file-only value. A negative value already in the file is displayed at the slider's minimum, and changing the control writes a non-negative value.

The app also shows a warning if the global profile or the selected profile contains `enabled = false`. In that state the grade is paused; remove the key to enable grading again.

### External profiles

A profile path outside the vkSumi config directory is opened read-only. This prevents the app from silently changing a GOverlay or custom-managed config. The window shows an **External config** warning and **Reset overrides** is disabled.

If you want the app to manage the file, move it under the vkSumi config directory and point vkSumi at it with `VKSUMI_CONFIG_FILE`, or use the normal per-executable path. See [External profiles](CONFIGURATION.md#external-profiles).

## Tray, close, and autostart

The app registers a StatusNotifierItem tray icon when a tray host is available. Its menu contains:

- **Show VibranceGUI**
- **Refresh Steam library**
- **Quit**

### X11

Closing the window hides it to the tray. `Ctrl+Q` quits. The first close sends a desktop notification.

### Wayland

The window toolkit cannot hide a Wayland window, so the app starts a small tray-only helper process and closes the main window. Use the tray's **Show** action to launch the app again, or **Quit** to stop the helper. The helper is started with `--tray-only`.

### Without a tray host

If the app cannot register a tray icon, close-to-tray is not active and the window closes normally. Run `vibrancegui --no-tray` to disable the tray explicitly and always use normal close behavior.

### Autostart

The **Start VibranceGUI with my desktop session** checkbox writes:

```text
$XDG_CONFIG_HOME/autostart/vibrancegui.desktop
```

or `~/.config/autostart/vibrancegui.desktop` when `XDG_CONFIG_HOME` is unset. The entry starts `vibrancegui --tray-only`, so it waits in the tray instead of opening the full window. **Show** in the tray opens the app. If no tray host is available, the helper cannot provide a usable menu.

## Command-line options

| Option | Effect |
| --- | --- |
| `--doctor` | Print vkSumi, Steam, account, installed-game, and launch-option diagnostics, then exit. |
| `--doctor --observe` | Also print currently observed vkSumi processes, AppIDs, executable names, layer state, and effective config paths. `--observe` only takes effect with `--doctor`. |
| `--no-tray` | Do not create the tray icon; closing the window closes the app. |
| `--light` / `--dark` | Select the startup palette when the saved theme is **System default**. A saved **Dark** or **Light** choice takes precedence. |
| `--hidden` | Start hidden if a tray was created. |
| `--tray-only` | Internal helper used for close-to-tray and autostart. It waits for the tray's **Show** action and is not intended as a normal launcher. |

Run from a build tree as `./target/release/vibrancegui ...`; run an installed package as `vibrancegui ...`.

## Files and data

Paths use the XDG base directories. The defaults are shown; set `XDG_CONFIG_HOME` or `XDG_CACHE_HOME` to relocate them.

| Path | Purpose |
| --- | --- |
| `~/.config/vkSumi/vkSumi.conf` | Global vkSumi profile. The default-vibrance slider writes `vibrance` here. |
| `~/.config/vkSumi/games/<executable-stem>.conf` | Normal per-executable vkSumi profile. |
| `~/.config/vibrancegui/state.json` | Tracked games, manual games, remembered profile paths, and theme. |
| `~/.config/vibrancegui/backups/` | Timestamped profile backups, capped at 20 files. |
| `~/.cache/vibrancegui/artwork/` | Downloaded Steam artwork. |
| `~/.cache/vibrancegui/icons/` | Icons extracted from Windows `.exe` files. |
| `$XDG_CONFIG_HOME/autostart/vibrancegui.desktop` | Optional autostart entry. |

The app does not create a log file. Status messages appear in the footer, and CLI diagnostics are printed to the terminal.

## Diagnostics

Run:

```bash
vibrancegui --doctor
vibrancegui --doctor --observe
```

`--doctor` reports the vkSumi layer status, Steam root and account, installed games, launch options, and warnings. `--observe` adds the live process view.

When sharing diagnostics, redact:

- the Steam account ID;
- your game list, if private;
- home-directory paths and any `VKSUMI_CONFIG_FILE` values that contain personal paths.

## Uninstall

For an AUR install, remove the package with your AUR helper or `pacman`. For an AppImage, delete the AppImage file.

The app does not have a global uninstaller. To remove its data as well:

- delete `$XDG_CONFIG_HOME/vibrancegui/` (state and backups; default `~/.config/vibrancegui/`);
- delete `$XDG_CACHE_HOME/vibrancegui/` (artwork and icons; default `~/.cache/vibrancegui/`);
- delete `$XDG_CONFIG_HOME/autostart/vibrancegui.desktop` if you enabled autostart (default `~/.config/autostart/vibrancegui.desktop`).

Do not delete `~/.config/vkSumi/` unless you also want to remove your color profiles. Uninstalling VibranceGUI does not uninstall vkSumi.

## Getting help

See [Troubleshooting](TROUBLESHOOTING.md), then open an [issue](https://github.com/NasirA1991/vibrancegui-linux/issues) with the details described in [Contributing](../CONTRIBUTING.md#reporting-a-bug).
