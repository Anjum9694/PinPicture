# PinPicture

A Windows desktop utility to pin any picture to the desktop, floating, topmost, clickthrough, animated and transparent pictures supported.

This simple tool allows you to pin any picture to the desktop (topmost), it might help you to use as a guide for designing and using them as reference, might be to paint, draw over (clickthrough), etc.

![snap01](https://raw.githubusercontent.com/vhanla/PinPicture/main/gitassets/snap01.png)

> **v1.3 — Rust port.** The app is now a small native Rust program (eframe/egui, no runtime
> dependencies). The original Delphi / FireMonkey-Skia sources are kept in [`delphi/`](delphi/),
> and a prebuilt Windows x64 binary is bundled at [`bin/PinPicture.exe`](bin/PinPicture.exe).
> Each pinned picture is its own light process, so several pins can run side by side.

These are the following picture formats currently supported:

- [x] BMP
- [x] JPEG
- [x] TIFF
- [x] GIF (animated)
- [x] PNG
- [x] WEBP
- [ ] FLIF
- [ ] SVG

## Features:

- [x] Pin (a.k.a. TopMost)
- [x] Clickthrough (restore click response on window activate a.k.a. alt-tab to it)
- [x] Alpha transparency level adjuster
- [x] Resizeable
- [x] Rotatable
- [x] Flip (horizontal/vertical)
- [ ] Color picker (for itself only)
- [x] 3D rotation included
- [ ] Slideshow
- [ ] Tabbed (selectable pictures)
- [ ] Follow mouse
- [ ] Keep picture on all Virtual Desktops
- [ ] Attach to window (visible only when you are working on a specific window)
- [x] Zoom and "crop" content
- [x] Minimize to system tray (each pin has its own tray icon — double-click it to bring the pin back on top (Minimize to Tray keeps the picture visible on the desktop, it just stops floating), right-click for Hide/Show picture, Toggle Topmost, Toggle Click-through, Exit)
- [x] Multiple pictures at once (the Open dialog accepts multi-selection; each extra picture opens its own pin window)
- [x] Auto start on Windows logon ("Auto Start" checkbox in the settings panel, stored in the HKCU Run key)
- [ ] Control panel accessible from the system tray icon
- [ ] Color picker system wide also included
- [ ] Rulers and guides included

## USAGE:

To zoom picture, press Ctrl key then the mouse wheel (or pinch). Double-click the picture to hide/show the button bar. Drag the window by the bar; resize with the red dot at the bottom-right.

To pin several pictures at once, hold Ctrl/Shift in the Open dialog and select them: the first one loads into the current window, every other one opens in a new pin window (cascaded). Each window has its own alpha, rotation and clickthrough settings; closing a window closes only that picture.

The minus button in the title bar minimizes the picture to the system tray. Double-click the tray icon to bring it back; right-click the tray icon for the menu.

You can also start a pin from the command line (or via `Open with`):

```
PinPicture.exe --file C:\path\picture.png --x 100 --y 100
```

e.g. Using as an overlay clickthrough guide for modeling 3d assets:
![snap01](https://raw.githubusercontent.com/vhanla/PinPicture/main/gitassets/snap02.png)

## BUILD (Rust version):

```
cargo build --release
```

The result is `target\release\PinPicture.exe` (Windows x64, ~8 MB, static). Auto Start (the checkbox in the settings panel) registers the exe under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

## KNOWN BUGS (original Delphi/FMX version):

- It is included effects features, like hue, brightness, blur, and more, but they're disabled since they consume a lot of memory.
- Sometimes, going back to clickable mode after rotating in 3d mode, the trackbars don't use the original coordinates.

## SUPPORT:

If you find this tool useful, and want me to continue its support, I'd appreciate your support.
