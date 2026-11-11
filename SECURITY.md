# Security policy

VibranceGUI for Linux is a local desktop utility that reads game/process metadata and edits user-owned vkSumi profiles. It does not provide a network service and does not run with elevated privileges.

## Supported versions

Security fixes are applied to the latest 0.2.x release. Older releases may not receive fixes; use the [latest release](https://github.com/NasirA1991/vibrancegui-linux/releases/latest) when reporting a problem.

## Reporting a vulnerability

Do not open a public issue containing exploit details, credentials, or private paths.

Use the repository's **Security → Report a vulnerability** form to contact the maintainer privately. If private reporting is unavailable, open a minimal public issue asking for a private contact channel; do not include the vulnerability details in that issue.

Include:

- affected version;
- impact and prerequisites;
- reproduction steps;
- relevant logs or file paths, redacted;
- whether the issue is in this project or in a dependency (vkSumi, Steam, a desktop portal, or the Vulkan driver).

There is no bug bounty program and no guaranteed response time.

## Security model

### Privileges and writes

- The app runs as the desktop user. It does not use `sudo`, polkit, or a privileged helper.
- It does not install packages, modify the Vulkan loader, or write Steam configuration.
- It writes only:
  - profiles inside the vkSumi config directory;
  - its own state file;
  - profile backups;
  - artwork and icon caches;
  - an optional autostart desktop entry.
- Profiles outside the vkSumi config directory are opened read-only by design.

### Process observation

While running, the app scans `/proc` and reads process environments. It retains only `ENABLE_VKSUMI`, `DISABLE_VKSUMI`, `SteamAppId`/`SteamGameId`, and `VKSUMI_CONFIG_FILE`; it does not log or persist unrelated environment variables. `/proc` access can be restricted by the kernel (`hidepid`), a sandbox, or a container, in which case detection is incomplete.

### Network

The app makes HTTPS requests to `cdn.cloudflare.steamstatic.com` to download missing Steam artwork. There is no telemetry, analytics, update check, account login, or remote configuration service.

### Subprocesses and desktop integration

- `xdg-open` is used for **Open config**, **Open config folder**, and About links.
- `notify-send` is used for the first close-to-tray notification; if it is unavailable, the app continues.
- On Wayland, close-to-tray starts a separate `vibrancegui --tray-only` helper.
- **Add manually** uses the desktop portal through `rfd`; the portal controls file access.
- The tray uses StatusNotifierItem over D-Bus. Its security properties depend on the desktop's tray host.

### Diagnostics

`--doctor` prints the Steam account ID, installed game names, launch options, layer status, and profile paths to the terminal. `--doctor --observe` additionally prints process IDs, AppIDs, executable names, mapped state, and effective config paths. Treat this output as personal data and redact it before posting publicly.

## Third-party components

- **vkSumi** is a separate project and is not bundled. Its layer runs inside games and is outside this app's trust boundary.
- **Steam** libraries, `localconfig.vdf`, and shortcuts are read as untrusted local input. Parsing errors should fail closed (skip the file/game), not execute content.
- **Steam artwork** is fetched over HTTPS and treated as image data; malformed images are handled by the image decoder.
- **Windows `.exe` icons** are parsed by `icoextract_rs`; malformed files should be handled as unsupported input.

Report vulnerabilities in those components to their respective projects.

## Release artifacts

GitHub releases include a native binary, an AppImage, and `SHA256SUMS`. Verify the checksum before running a downloaded artifact. The project does not currently publish GPG signatures; checksums detect accidental corruption, not a compromised release account.

The AUR source package builds from the tagged archive. The AUR `-bin` package uses the release binary. Review AUR package changes as you would any other distribution package.

## Secrets in the repository

No AUR credentials, tokens, or signing keys are stored in the repository. The release workflow expects `AUR_USERNAME`, `AUR_EMAIL`, and `AUR_SSH_PRIVATE_KEY` as repository secrets; protect those secrets and rotate the SSH key if it is exposed.
