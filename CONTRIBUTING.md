# Contributing

Thanks for helping improve VibranceGUI for Linux. This project is a small, unofficial vkSumi companion; focused bug reports, compatibility findings, and documentation fixes are especially useful.

## Before you start

- Check [existing issues](https://github.com/NasirA1991/vibrancegui-linux/issues) and the [roadmap](docs/DEVELOPMENT.md#roadmap) before opening a feature request.
- Read the [user guide](docs/USAGE.md) and [troubleshooting](docs/TROUBLESHOOTING.md); many setup problems are covered there.
- Keep the project's safety scope: Steam files stay read-only, external profiles stay read-only, and the app does not run as root.

## Reporting a bug

Open an issue with:

- app version (footer, package version, or release tag);
- distribution and desktop session (Wayland or X11);
- how vkSumi is installed (AUR, Flatpak, manual);
- what you expected and what happened;
- the game's launch options, with paths and account details redacted;
- relevant output from:

  ```bash
  vibrancegui --doctor
  vibrancegui --doctor --observe
  ```

`--doctor` prints your Steam account ID, installed games, and launch options. Redact anything you do not want public. Do not paste tokens, passwords, or private file contents.

For a rendering problem, include:

- whether the game window says **layer mapped** or **layer unconfirmed**;
- whether the game is Vulkan;
- whether HDR is enabled;
- whether anti-cheat is present.

## Feature requests

Describe the workflow you are trying to achieve, not only the control you want added. Useful context includes the launcher, game, desktop, and why the existing **Add manually** or vkSumi config file is insufficient.

Features that require writing Steam configuration need a design for backups, rollback, and Steam-closed behavior before implementation. Do not assume VDF writing is a small change.

## Pull requests

1. Fork the repository and create a focused branch.
2. Make the change and add tests where behavior is testable without a real Steam library or Vulkan game.
3. Run the checks below.
4. Update the relevant documentation in the same pull request.
5. Open the pull request with a short description of the problem, the approach, and any manual testing performed.

Keep unrelated refactors and formatting churn out of a bug-fix pull request. For UI changes, include a screenshot or a short recording when practical.

### Checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
```

CI runs the same checks on Ubuntu 24.04. See [Development](docs/DEVELOPMENT.md#build) for native dependencies.

### Design constraints

- Keep the desktop UI on **egui/eframe**. A GTK port was evaluated and rejected; do not reintroduce it without a maintainer decision.
- Do not add managed Steam launch-option writing as an incidental change. It needs explicit backup, rollback, and drift handling.
- Do not write profiles outside the vkSumi config directory. The read-only external-profile behavior is intentional.
- Do not add a daemon, root helper, telemetry, or an auto-update mechanism without prior discussion.
- Avoid new dependencies when the standard library or an existing crate is sufficient. Explain any new dependency in the pull request.
- Preserve the unofficial naming and disclaimer. Do not present the project as the official Windows VibranceGUI or as affiliated with it.
- Do not include personal Steam account data, home paths, or downloaded artwork in tests or fixtures.

### Commit messages

Short, imperative commit subjects are preferred, for example:

```text
tray: keep helper alive after Wayland close
docs: document VKSUMI_CONFIG_FILE precedence
```

## Documentation

Documentation lives in:

- [README.md](README.md) — entry point and quick start
- [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) — installation and first profile
- [docs/USAGE.md](docs/USAGE.md) — UI workflows
- [docs/CONFIGURATION.md](docs/CONFIGURATION.md) — paths, precedence, controls
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) — common failures
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — build, architecture, release process

If a change affects a command, path, flag, control, package, or release step, update the matching document and verify relative links.

## Licensing

By contributing, you agree that your contribution is licensed under the project's MIT license. There is no separate contributor license agreement.
