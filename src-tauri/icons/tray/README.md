# Tray icon

`music.svg` is the user-supplied white musical-note artwork (kept unchanged).
These transparent icons are only used in the system tray; application/Dock icons
remain `../icon.png` and `../icon.ico`.

Regenerate the PNGs from the repository root with the existing Tauri CLI:

```sh
node_modules/.bin/tauri icon src-tauri/icons/tray/music.svg \
  --output src-tauri/icons/tray \
  --png 16 --png 22 --png 24 --png 32 --png 48 --png 64
```

Linux exports multiple sizes as StatusNotifierItem ARGB pixmaps without an icon
theme name (which would override this artwork). Windows uses the embedded 32px
PNG. The white color is intentional; no colored tile/background is added.
