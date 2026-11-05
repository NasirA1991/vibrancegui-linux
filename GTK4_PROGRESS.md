# GTK4 parity progress

Branch: `gtk4`

- [x] Native GTK4 window/header
- [x] Theme selection (system/dark/light)
- [x] About dialog/version/GitHub link
- [x] Game grid filtered to vkSumi-enabled Steam games, shortcuts and manual games
- [x] Steam game picker
- [x] Add manually via GTK file chooser
- [x] Remove game
- [x] Inline game page (no child window)
- [x] Basic/advanced vkSumi sliders with config save
- [x] `.exe` icon extraction
- [x] Steam artwork download/cache
- [x] Autostart toggle
- [x] StatusNotifierItem tray with Show/Quit and pixmap icon
- [x] Startup+runtime process adoption (restarts UI to refresh)
- [x] Refresh/Reload actions (restart UI to reload state/config)
- [ ] AUR packaging switched to GTK dependencies
- [ ] Final parity verification on a real desktop session
- [ ] Replace main/AUR binary after verification

Known limitations:
- UI refresh after process adoption/config reload currently restarts the app.
- Runtime testing has been under Xvfb; tray/portal behavior on Wayland needs a real session check.
