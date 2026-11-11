# Troubleshooting

This page covers the problems reported most often. For the first-run sequence, start with [Getting Started](GETTING_STARTED.md). Otherwise, start with the symptom that matches; each section ends with the diagnostic command that gives the most useful next clue.

## vkSumi is not detected

**Symptom:** the header says **vkSumi not detected**, or the setup banner says vkSumi is not installed.

vkSumi is a separate Vulkan layer. Neither AUR package installs it automatically:

```bash
paru -S vksumi
```

If you use another installation method, make sure the layer is visible to the game. Common cases:

- **AUR package installed, header still says not detected:** press **Refresh**. The check runs at startup and on refresh.
- **Flatpak vkSumi or a Flatpak game runtime:** the layer may be visible only inside the sandbox. The app's host-side check can fail while the running game is still graded. A game tile showing **Detected · vkSumi layer mapped** is stronger evidence than the header.
- **Manual or custom-prefix install:** the manifest and library must be discoverable from the host. Check that `/usr/share/vulkan/implicit_layer.d/vksumi.json` (or your prefix's equivalent) exists and that its `library_path` points to an existing library.
- **vksumi installed for the wrong architecture:** a 64-bit game needs the 64-bit layer. Install the matching package or build.
- **The app was started before vkSumi was installed:** restart it or press **Refresh**.

Run:

```bash
vibrancegui --doctor
```

The output lists the manifest, library, and any validation problem. Fix the layer first; profile editing is still possible without it, but grading will not happen.

## The game is missing from the list

The normal Steam view is intentionally **enabled-first**: in the filtered view, a regular Steam game appears only when its launch options contain the exact token `ENABLE_VKSUMI=1`, or when it is running with vkSumi enabled. **Add** can make any installed game visible regardless of the filter.

Try these in order:

1. Click **Show all** to see the whole installed library.
2. In Steam, open **Properties → Launch Options** and check the exact token. `XENABLE_VKSUMI=1`, `ENABLE_VKSUMI=0`, and `ENABLE_VKSUMI=1` inside an unrelated argument do not count.
3. Press **Refresh**. The app re-reads the library and launch options.
4. If the game is a Steam shortcut, confirm it exists in Steam's shortcut list. Shortcuts are tracked on first run and remain visible even when not enabled.
5. If the game is not a Steam game, launch it once with `ENABLE_VKSUMI=1` and the app open, or use **Add manually**.
6. If Steam itself is not found, see [Steam library or account is not found](#steam-library-or-account-is-not-found).

Adding a game with **Add** does not add the launch option. It only makes the game editable in the app.

### Flatpak Steam

The library scanner targets native Steam roots. Flatpak Steam libraries are not imported. A Flatpak game can still be auto-adopted as a running process when its environment and executable are visible, and **Add manually** remains available. See [the user guide](USAGE.md#adding-games).

## The game runs but colours do not change

First check the game window's status line:

- **Detected · vkSumi layer mapped** — the layer is in the process. Continue with the checks below.
- **Detected · layer unconfirmed** — `ENABLE_VKSUMI=1` is present, but `libVkLayer_vksumi.so` is not visible in `/proc/<pid>/maps`, or the maps could not be read. The app cannot confirm that the layer loaded.

Check, in this order:

1. **`DISABLE_VKSUMI=1`** in the launch options or environment. An explicit disable wins. Remove it.
2. **`enabled = false`** in the selected profile or the global profile. The game window shows a warning when either is set; remove the key or set it to `true`.
3. **The profile path.** The app may be editing a remembered profile that is not the one vkSumi opened. Confirm the path shown at the bottom of the game window and compare it with the process's `VKSUMI_CONFIG_FILE`. See [Profile resolution](CONFIGURATION.md#profile-resolution).
4. **Vulkan.** vkSumi is a Vulkan layer. OpenGL games, GL-only renderers, and some compatibility modes are not supported.
5. **Anti-cheat.** Some games block all implicit Vulkan layers or specific layer libraries. Do not try to bypass anti-cheat; use the game without vkSumi.
6. **The game was already running before the launch option was added.** Restart the game so the environment and layer are picked up.

For a deeper look:

```bash
vibrancegui --doctor --observe
```

`--observe` shows each detected PID, AppID, executable, `mapped` state, and effective config path. If a game is absent there, the app cannot see its environment; check `/proc` permissions and whether the game runs in a different namespace or sandbox.

## The profile cannot be linked or is read-only

### No linked profile yet

For a normal Steam game, the app needs one of these:

- a valid `VKSUMI_CONFIG_FILE` in the launch options;
- a `games/<executable-stem>.conf` profile for a Steam shortcut;
- a running process observed with vkSumi enabled;
- a remembered profile path from a previous run.

Launch the game once with VibranceGUI open. The app records the path reported by the process. If the game uses a launcher that starts the real game in a child process, make sure the child carries the environment variable and is visible to `/proc`.

### External config warning

A config path outside the vkSumi config directory is intentionally read-only. This protects profiles managed by GOverlay or custom setups from silent edits.

To make it writable, use a profile under the vkSumi directory and point the game at it. For example, add an explicit variable to the launch options:

```text
ENABLE_VKSUMI=1 VKSUMI_CONFIG_FILE=/home/you/.config/vkSumi/games/my-game.conf %command%
```

The path must be absolute (or use a form vkSumi expands). Then relaunch the game. The app shows the external warning only when the resolved path is outside its managed directory.

### Manual game matching fails

For a manually added game, the app matches the running process by executable file name or stem. Check that:

- the **Executable** field points to the file that actually runs;
- the running process has `ENABLE_VKSUMI=1`;
- the process is visible in `/proc` (not hidden by a sandbox or `hidepid`).

If the executable is a launcher that starts a different binary, add the real game binary instead. The optional **vkSumi config** field lets you force a known profile path.

## Settings do not save

The app writes profiles after a short debounce and shows **saved** in the footer. If changes do not persist:

1. Check for the **External config** warning. External profiles are read-only.
2. Check the profile path at the bottom of the game window. It should be under the vkSumi config directory.
3. Check file and directory permissions; the app runs as your user and does not use root.
4. Check free disk space. A write failure is shown in the footer.
5. If you edited the file in another program while the game window was open, select another game and return, or press **Refresh**, to reload it.

Backups are capped at 20 files. A full backup directory does not prevent a save; the oldest backup is removed. See [Backups](CONFIGURATION.md#backups).

The app never writes Steam launch options, so a missing launch option is not a save failure.

## Steam library or account is not found

The app searches native Steam locations:

- `~/.steam/steam`
- `~/.local/share/Steam`
- `$XDG_DATA_HOME/Steam`

It reads `libraryfolders.vdf` for additional library folders and uses the newest `localconfig.vdf` under `userdata/` for launch options. If you have multiple local Steam accounts, the newest file is used. If that file cannot be read or parsed, launch options are unavailable; the app does not fall back to an older account's file.

Check:

```bash
vibrancegui --doctor
```

The output shows the root, account ID, game count, and warnings. The account ID is a local identifier, not a password or token.

Common causes:

- **Flatpak Steam only:** the library is not scanned. Use **Add manually** or auto-adoption.
- **Custom Steam root:** point the app at a normal Steam root or use manual add.
- **Steam is installed but libraries are not mounted:** check the paths in `libraryfolders.vdf`.
- **A game is installed outside a registered library:** add it manually.

## The tray icon is missing or close does not work

The tray uses StatusNotifierItem (SNI). A host is required:

- **KDE Plasma:** SNI support is built in.
- **GNOME:** install and enable an AppIndicator/StatusNotifier extension.
- **Other desktops:** check for a compatible SNI host.

If no host is available, the app does not get a tray handle. Closing the window then closes the app instead of hiding it.

On Wayland, close-to-tray starts `vibrancegui --tray-only` and closes the main window. If the helper cannot be started, the app leaves the window open and shows a warning in the footer. The first successful close may send a desktop notification; `notify-send` is optional and its absence does not affect the app.

If you do not want a tray, start with:

```bash
vibrancegui --no-tray
```

`Ctrl+Q` always quits. If an autostart entry was enabled, remove `$XDG_CONFIG_HOME/autostart/vibrancegui.desktop` (or `~/.config/autostart/vibrancegui.desktop`) to stop the tray helper at login.

## The app does not start

Run it from a terminal to see the error:

```bash
vibrancegui
```

Check:

- a graphical session is available (`DISPLAY` or `WAYLAND_DISPLAY` is set);
- GL/EGL and X11/Wayland libraries are installed;
- the AppImage has execute permission;
- you are not running it as root.

For an AppImage:

```bash
chmod +x vibrancegui-linux-*.AppImage
./vibrancegui-linux-*.AppImage
```

If FUSE is unavailable, use the AppImage's extract-and-run fallback:

```bash
./vibrancegui-linux-*.AppImage --appimage-extract-and-run
```

The AppImage does not bundle Vulkan drivers, vkSumi, or a desktop portal. See [AppImage does not start](#appimage-does-not-start).

## AUR or source build fails

### Use the prebuilt package first

`vibrancegui-linux-bin` installs the release binary and does not compile Rust on your machine:

```bash
paru -S vibrancegui-linux-bin
```

This is the quickest way to determine whether a build-environment problem is the cause.

### Source build dependencies

The source package needs Rust and the native libraries listed in the [development guide](DEVELOPMENT.md#build). On Arch, the relevant packages include:

```bash
sudo pacman -S --needed base-devel rust pkg-config openssl \
  libglvnd wayland libxkbcommon libx11 libxcursor libxi libxrandr libxinerama fontconfig
```

Build manually with:

```bash
cargo build --release --locked
```

Check the first compiler or linker error, not the last line. Common causes are a missing OpenSSL development package, missing X11/Wayland headers, an outdated Rust toolchain, or a full build directory.

If an AUR helper reports that it cannot find the package, refresh its databases and retry. Do not install the source and `-bin` packages at the same time; they conflict.

## AppImage does not start

Check, in order:

1. **Execute permission:** `chmod +x vibrancegui-linux-*.AppImage`.
2. **FUSE:** install FUSE 2 compatibility, or run with `--appimage-extract-and-run`.
3. **Host libraries:** install GL/EGL, X11/Wayland, fontconfig, and OpenSSL runtime libraries. The AppImage uses host libraries by design.
4. **Desktop portal:** **Add manually** needs `xdg-desktop-portal` and a portal backend; the rest of the app can run without it.
5. **vkSumi:** the AppImage does not install the layer. Install it separately.

A missing library is usually printed in the terminal when started from a shell.

## Anti-cheat or game crash

Some games block implicit Vulkan layers, and some anti-cheat systems terminate a process when an unknown layer is present. That is a game/anti-cheat policy, not a VibranceGUI setting.

To disable the layer for one game:

- remove `ENABLE_VKSUMI=1` from its launch options, or
- add `DISABLE_VKSUMI=1`.

Do not attempt to bypass anti-cheat protections. Use the game without vkSumi if it is not compatible.

## Reset app data

Close VibranceGUI first. To reset only the app's tracking and remembered paths, remove:

```text
$XDG_CONFIG_HOME/vibrancegui/state.json
```

When `XDG_CONFIG_HOME` is unset, the path is `~/.config/vibrancegui/state.json`. This does not delete vkSumi profiles. To remove caches:

```text
$XDG_CACHE_HOME/vibrancegui/
```

When `XDG_CACHE_HOME` is unset, the path is `~/.cache/vibrancegui/`.

To remove the optional autostart entry:

```text
$XDG_CONFIG_HOME/autostart/vibrancegui.desktop
```

When `XDG_CONFIG_HOME` is unset, the autostart path is `~/.config/autostart/vibrancegui.desktop`.

Back up profiles in `~/.config/vkSumi/` before deleting anything there. The app's backups are in `$XDG_CONFIG_HOME/vibrancegui/backups/` (default `~/.config/vibrancegui/backups/`).

## Collect diagnostics

```bash
vibrancegui --doctor
vibrancegui --doctor --observe
```

Include in a bug report:

- app version (the footer, package version, or release tag);
- distribution and desktop session (Wayland or X11);
- whether vkSumi was installed through AUR, Flatpak, or manually;
- the relevant `--doctor` lines;
- the game's launch options, with personal paths redacted.

Redact your Steam account ID, game list, home directory, and any `VKSUMI_CONFIG_FILE` value that contains personal information. See [Reporting a bug](../CONTRIBUTING.md#reporting-a-bug).
