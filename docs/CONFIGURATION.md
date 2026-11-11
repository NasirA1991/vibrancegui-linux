# Configuration reference

VibranceGUI for Linux edits the same plain-text vkSumi configuration files that vkSumi reads. This page documents:

- where profiles live and how the app chooses one;
- how global and per-game values interact;
- every numeric control exposed by the UI;
- files the app creates;
- how external profiles are handled.

For the UI workflow, see the [user guide](USAGE.md). For symptoms and fixes, see [Troubleshooting](TROUBLESHOOTING.md).

## File locations

Paths follow the XDG base-directory specification. The defaults are shown.

| File | Default path | Written by the app |
| --- | --- | --- |
| Global vkSumi profile | `~/.config/vkSumi/vkSumi.conf` | Only `vibrance` from the default slider; other existing keys are preserved |
| Per-game vkSumi profile | `~/.config/vkSumi/games/<executable-stem>.conf` | Numeric controls edited in the game window |
| App state | `~/.config/vibrancegui/state.json` | Tracked games, manual games, remembered profile paths, theme |
| Profile backups | `~/.config/vibrancegui/backups/` | Timestamped copies before a profile is replaced |
| Artwork cache | `~/.cache/vibrancegui/artwork/` | Downloaded Steam artwork |
| Icon cache | `~/.cache/vibrancegui/icons/` | Icons extracted from Windows executables |
| Autostart entry | `$XDG_CONFIG_HOME/autostart/vibrancegui.desktop` | Optional tray-helper entry |

`<executable>` is the executable **stem**: `juvio.exe` becomes `juvio.conf`, not `juvio.exe.conf`.

The app uses `$XDG_CONFIG_HOME` and `$XDG_CACHE_HOME` when set. If they are unset, the paths above default to `~/.config` and `~/.cache`.

## Profile resolution

When you select a game, the app chooses a profile path in this order:

1. **Manual profile override.** For a manually added game, the **vkSumi config** field from the add dialog.
2. **Remembered profile.** A path previously observed and saved in the app state. A remembered path outside the vkSumi directory is used only if it still exists.
3. **Steam launch-option variable.** `VKSUMI_CONFIG_FILE` parsed from the game's launch options.
4. **Steam shortcut path.** For a Steam shortcut, `games/<executable-stem>.conf` under the vkSumi config directory.
5. **Live process path.** For a running game, the path from its `VKSUMI_CONFIG_FILE`, or `games/<executable-stem>.conf` when the variable is absent.

For a running process, the process environment is the most direct evidence of the profile vkSumi actually opened. The app stores that path in its state so you can edit it later without the game running.

The app supports `~/`, `$HOME`, and `$USER` expansion when reading `VKSUMI_CONFIG_FILE` values from Steam launch options or a process environment. For a process environment, a relative value is resolved against that process's working directory when it is readable. A relative value in Steam launch options is expanded but is not resolved against the game's working directory. The **Add manually** config field is used as an absolute path and is not expanded.

If no path resolves, the game window says **No linked vkSumi profile yet**. Launch the game once with the app open, or add a manual profile path for a non-Steam game.

### Per-executable collisions

vkSumi's default profile naming is based on the executable name. If two games share an executable name (for example, two Proton games that both run `game.exe`), they share the same default profile. Set a distinct `VKSUMI_CONFIG_FILE` for each game to avoid this.

## Global and per-game values

vkSumi merges a per-game profile over the global profile. The app follows the same model:

- **Global profile** — applies to games that do not override a value.
- **Per-game profile** — stores the values you changed for that game. Unchanged controls inherit the global value.
- **reset** on one control removes that key from the per-game profile, restoring inheritance.
- **Reset overrides** removes every key managed by the editor from the per-game profile. It does not remove unrelated keys.

The **Default Vibrance Level** slider writes only `vibrance` to the global profile. It does not copy other global values into game profiles.

A value shown in muted text in the game window is inherited. A value shown in normal text is stored in that game's profile. The main window uses a 0–100% display for the global vibrance control; the file stores the equivalent 0.0–1.0 value.

If either profile contains `enabled = false`, the app displays a warning. The profile still exists and can be edited, but vkSumi's grading may be paused until the effective `enabled` value is removed or set to `true`.

Negative vibrance is not exposed by the slider or the numeric editor; it remains a config-file-only value. Reading a file with a negative value displays it at the slider's minimum, so use a text editor if you specifically need a negative value.

## Numeric controls

The UI exposes 27 numeric keys. **Vibrance** is in Basic; the rest are under **Advanced (all vkSumi sliders)**.

### Basic

| Key | Label | Range | Notes |
| --- | --- | --- | --- |
| `vibrance` | Vibrance | 0.0–1.0 | Smarter saturation: boosts colorful areas while limiting skin-tone washout |

### Color and tone

| Key | Label | Range | Notes |
| --- | --- | --- | --- |
| `saturation` | Saturation | -1.0–1.0 | Overall color intensity |
| `temperature` | Temperature | -1.0–1.0 | Warm (+) or cool (-) white balance |
| `contrast` | Contrast | -1.0–1.0 | Difference between light and dark areas |
| `brightness` | Brightness | -1.0–1.0 | Overall brightness |
| `gamma` | Gamma | -1.0–1.0 | Midtones without moving pure black or white |
| `exposure` | Exposure | -3.0–3.0 | Exposure in EV stops |
| `shadows` | Shadows | -1.0–1.0 | Darkest tonal range |
| `midtones` | Midtones | -1.0–1.0 | Middle tonal range |
| `highlights` | Highlights | -1.0–1.0 | Brightest tonal range |

### Channel gains and hue shifts

| Key | Label | Range | Notes |
| --- | --- | --- | --- |
| `red_gain` | Red gain | -1.0–1.0 | Red channel scale |
| `green_gain` | Green gain | -1.0–1.0 | Green channel scale |
| `blue_gain` | Blue gain | -1.0–1.0 | Blue channel scale |
| `hue_deg` | Hue rotation (°) | -180–180 | Rotates all hues |
| `red_hue` | Red hue | -1.0–1.0 | Shifts only red hues |
| `green_hue` | Green hue | -1.0–1.0 | Shifts only green hues |
| `blue_hue` | Blue hue | -1.0–1.0 | Shifts only blue hues |
| `cyan_hue` | Cyan hue | -1.0–1.0 | Shifts only cyan hues |
| `magenta_hue` | Magenta hue | -1.0–1.0 | Shifts only magenta hues |
| `yellow_hue` | Yellow hue | -1.0–1.0 | Shifts only yellow hues |

### Split toning, vignette, and daltonize

| Key | Label | Range | Notes |
| --- | --- | --- | --- |
| `shadow_tint` | Shadow tint | 0.0–1.0 | Strength of colored shadow tinting |
| `shadow_tint_hue` | Shadow tint hue (°) | 0–360 | Hue used for shadow tinting |
| `highlight_tint` | Highlight tint | 0.0–1.0 | Strength of colored highlight tinting |
| `highlight_tint_hue` | Highlight tint hue (°) | 0–360 | Hue used for highlight tinting |
| `vignette` | Vignette | 0.0–1.0 | Darkens frame edges |
| `daltonize_severity` | Daltonize severity | 0.0–1.0 | Strength of color-vision-deficiency correction |
| `daltonize_strength` | Daltonize strength | 0.0–1.0 | Overall strength of the daltonize effect |

### Keys not exposed

Keys that are not in the table above are preserved and remain editable in the config file. This includes `enabled`, `compare`, the daltonize mode, and censor-box settings. The editor only appends, updates, or removes the numeric keys it exposes.

## File format and writes

The editor is line-oriented and deliberately conservative:

- It reads active `key = value` lines and ignores comments and blank lines.
- Existing active lines are updated in place.
- A missing key is appended at the end.
- Commented template lines are left untouched.
- Inline comments are removed when a line is updated; the value is rewritten as `key = 0.000`.
- **reset** removes the active line rather than writing a default value.
- Saving writes a temporary file in the same directory, then renames it over the profile.
- If the profile already exists, a backup is copied first.

The app only writes a game profile when it is inside the vkSumi config directory. It does not write Steam files, vkSumi layer files, or the game executable.

## Backups

Backups are stored in `~/.config/vibrancegui/backups/` (or the XDG equivalent) and named:

```text
<profile-name>.<unix-timestamp>.bak
```

At most one backup is created per second for rapid slider changes. The directory is capped at 20 `.bak` files; the oldest are removed first. Backups contain the previous profile contents, not the app state.

To restore a backup, close the game (vkSumi may still reload an open profile), copy the backup over the profile, then reopen the game settings in the app or press **Refresh**.

## App state

`~/.config/vibrancegui/state.json` stores:

- tracked Steam AppIDs;
- manually added and auto-adopted games;
- remembered effective profile paths;
- theme choice;
- first-run initialization flags.

The state file contains no game launch options, environment dumps, or account credentials. It is written atomically. Removing it resets the app's tracking list and remembered paths; it does not delete profiles.

## Autostart entry

Enabling autostart writes an XDG-style desktop entry under `$XDG_CONFIG_HOME/autostart/` (default `~/.config/autostart/`). Its command is:

```text
vibrancegui --tray-only
```

The helper displays a tray icon and starts the main app when **Show** is selected. If no StatusNotifierItem host is available, the helper cannot provide a usable menu.

## External profiles

A profile outside the vkSumi config directory is opened read-only. This is intentional: GOverlay, a manual installation, or another tool may own that file, and the app should not silently rewrite it.

The warning appears when:

- a manual game's **vkSumi config** field points outside the managed directory;
- `VKSUMI_CONFIG_FILE` in the launch options or process environment points outside it;
- a remembered path is outside it.

The app can display the values and show the path, but the sliders, **reset**, and **Reset overrides** are disabled.

To let the app manage the profile, use a path under the vkSumi config directory and point the game at it. For example, put the profile at:

```text
~/.config/vkSumi/games/my-game.conf
```

and add this to the game's Steam launch options:

```text
ENABLE_VKSUMI=1 VKSUMI_CONFIG_FILE=/home/you/.config/vkSumi/games/my-game.conf %command%
```

Use an absolute path in the launch option, or a form vkSumi expands. The app expands `~/`, `$HOME`, and `$USER` when reading this variable from launch options or a process, but the **Add manually** field itself is used literally.

## Example profile

A minimal profile with a few overrides:

```ini
# vkSumi per-game profile
vibrance = 0.350
saturation = 0.100
contrast = 0.050
shadow_tint = 0.200
shadow_tint_hue = 220.000
```

Unlisted controls inherit from `~/.config/vkSumi/vkSumi.conf`. A control with no line can also be added through the UI, which appends it in the same format.
