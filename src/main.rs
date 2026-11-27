// PinPicture (Rust port) — pin any picture to the desktop: topmost,
// click-through, transparent, animated. One process per pinned picture.
//
// Original Delphi/FireMonkey program: https://github.com/vhanla/PinPicture
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, TextureOptions, Vec2};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconAttributes, TrayIconEvent};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "PinPicture";

// Segoe MDL2 Assets glyphs (same code points the Delphi version used)
const G_BARFLIP: char = '\u{E90B}'; // switch drag bar top/bottom
const G_INFO: char = '\u{E946}'; // about
const G_OPEN: char = '\u{E8E5}'; // open picture(s)
const G_SETTINGS: char = '\u{E9E9}'; // rotation/settings panel
const G_CLICK: char = '\u{E8B0}'; // click-through
const G_PIN_ON: char = '\u{E840}'; // topmost on
const G_PIN_OFF: char = '\u{E77A}'; // topmost off
const G_TRAY: char = '\u{E921}'; // minimize to tray (ChromeMinimize)
const G_CLOSE: char = '\u{E711}'; // close

const PERSPECTIVE: f32 = 900.0;
const BAR_H: f32 = 38.0;

/// Window-level flags shared between the UI thread and the tray polling thread.
#[derive(Clone, Copy)]
struct Flags {
    hidden: bool,
    topmost: bool,
    click_through: bool,
}

struct FrameImg {
    img: egui::ColorImage,
    dur: Duration,
}

struct PinImage {
    frames: Vec<FrameImg>,
    cur: usize,
    next_at: f64,
    tex: egui::TextureHandle,
    size_px: Vec2,
}

impl PinImage {
    fn current_dur(&self) -> Duration {
        self.frames[self.cur].dur
    }
}

/// Some Windows GL drivers cap textures at ~2048px — downscale bigger
/// pictures at load so they don't upload-fail into a blank (black/white) window
/// (egui adds a 2px atlas border, so we stay a bit below the cap).
const MAX_TEX: u32 = 1900;

fn fit_rgba(img: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) <= MAX_TEX {
        return img;
    }
    let s = MAX_TEX as f64 / w.max(h) as f64;
    let nw = ((w as f64 * s).round() as u32).max(1);
    let nh = ((h as f64 * s).round() as u32).max(1);
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

fn color_image(img: &image::RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    )
}

fn load_pin_image(ctx: &egui::Context, path: &Path) -> Option<PinImage> {
    let bytes = std::fs::read(path).ok()?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let frames: Vec<FrameImg> = if ext == "gif" {
        use image::AnimationDecoder;
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)).ok()?;
        dec.into_frames()
            .collect_frames()
            .ok()?
            .into_iter()
            .map(|f| {
                let dur: Duration = f.delay().into();
                let img = fit_rgba(f.into_buffer());
                FrameImg { img: color_image(&img), dur }
            })
            .collect()
    } else {
        let img = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?
            .to_rgba8();
        let img = fit_rgba(img);
        vec![FrameImg { img: color_image(&img), dur: Duration::MAX }]
    };
    if frames.is_empty() {
        return None;
    }
    let sz = frames[0].img.size;
    let size_px = Vec2::new(sz[0] as f32, sz[1] as f32);
    let tex = ctx.load_texture("pin", frames[0].img.clone(), TextureOptions::LINEAR);
    Some(PinImage { frames, cur: 0, next_at: 0.0, tex, size_px })
}

fn tray_icon_from_ico() -> Option<Icon> {
    let img = image::ImageReader::new(std::io::Cursor::new(
        include_bytes!("../assets/ico_white.ico").to_vec(),
    ))
    .with_guessed_format()
    .ok()?
    .decode()
    .ok()?
    .to_rgba8();
    let side = img.width().min(img.height());
    if side < 8 {
        return None;
    }
    Icon::from_rgba(img.to_vec(), img.width(), img.height()).ok()
}

fn autostart_get() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(RUN_KEY) {
        Ok(k) => matches!(k.get_value::<String, _>(RUN_NAME), Ok(ref v) if !v.is_empty()),
        Err(_) => false,
    }
}

fn autostart_set(enable: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let (k, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
        k.set_value(RUN_NAME, &format!("\"{}\"", exe.display())).map_err(|e| e.to_string())
    } else {
        if let Ok(k) = hkcu.open_subkey(RUN_KEY) {
            let _ = k.delete_value(RUN_NAME);
        }
        Ok(())
    }
}

/// Parsed command line: pin a file and/or a cascade position.
/// Accepts `--file <path>` or a bare path (Windows "Open with" passes a bare
/// path), plus `--x <f32>` / `--y <f32>`.
#[derive(Default)]
struct Args {
    file: Option<PathBuf>,
    x: Option<f32>,
    y: Option<f32>,
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--file" => a.file = it.next().map(PathBuf::from),
            "--x" => a.x = it.next().and_then(|v| v.parse().ok()),
            "--y" => a.y = it.next().and_then(|v| v.parse().ok()),
            other => {
                let cand = PathBuf::from(other);
                if a.file.is_none() && cand.is_file() {
                    a.file = Some(cand);
                }
            }
        }
    }
    a
}

struct PopupIds {
    showhide: tray_icon::menu::MenuId,
    topmost: tray_icon::menu::MenuId,
    click: tray_icon::menu::MenuId,
    exit: tray_icon::menu::MenuId,
}

fn build_popup() -> (Menu, PopupIds) {
    let menu = Menu::new();
    let show = MenuItem::with_id("showhide", "Hide / Show picture", true, None);
    let top = MenuItem::with_id("topmost", "Toggle Topmost", true, None);
    let click = MenuItem::with_id("click", "Toggle Click-through", true, None);
    let sep = PredefinedMenuItem::separator();
    let exit = MenuItem::with_id("exit", "Exit PinPicture", true, None);
    let ids = PopupIds {
        showhide: show.id().clone(),
        topmost: top.id().clone(),
        click: click.id().clone(),
        exit: exit.id().clone(),
    };
    let _ = menu.append_items(&[&show, &top, &click, &sep, &exit]);
    (menu, ids)
}

struct PinApp {
    file: Option<PathBuf>,
    image: Option<PinImage>,

    // picture transform
    zoom: f32,
    pan: Vec2,
    rot: [f32; 3],
    flip_h: bool,
    flip_v: bool,
    opacity: f32,

    // window flags (authoritative copy lives in `flags`)
    topmost: bool,
    click_through: bool,
    prev_focused: bool,

    // ui state
    bar_top: bool,
    bar_visible: bool,
    taskbar_done: bool, // explicit "button bar" toggle; forced on when no image
    settings_open: bool,
    about_open: bool,
    autostart: bool,
    autostart_err: Option<String>,
    pending_open: bool,
    last_outer: Option<Pos2>,
    spawn_pos: (f32, f32),

    // tray
    _tray: Option<TrayIcon>,
    flags: Arc<Mutex<Flags>>,
}

impl PinApp {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let ctx = &cc.egui_ctx;

        // Use Segoe MDL2 Assets if present, for the toolbar glyphs
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\SegoeIcons.ttf") {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("mdl2".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push("mdl2".to_owned());
            }
            ctx.set_fonts(fonts);
        }
        ctx.style_mut_of(egui::Theme::Dark, |s| {
            s.visuals.panel_fill = Color32::TRANSPARENT;
            s.visuals.override_text_color = Some(Color32::WHITE);
        });

        let image = args.file.as_deref().and_then(|p| load_pin_image(ctx, p));
        if image.is_none() {
            if let Some(f) = args.file.as_ref() {
                eprintln!("pin image failed to load: {}", f.display());
            }
        }
        let spawn_pos = (args.x.unwrap_or(100.0), args.y.unwrap_or(100.0));

        let mut app = PinApp {
            file: args.file,
            image,
            zoom: 1.0,
            pan: Vec2::ZERO,
            rot: [0.0, 0.0, 0.0],
            flip_h: false,
            flip_v: false,
            opacity: 1.0,
            topmost: true,
            click_through: false,
            prev_focused: true,
            bar_top: true,
            bar_visible: false,
            taskbar_done: false,
            settings_open: false,
            about_open: false,
            autostart: autostart_get(),
            autostart_err: None,
            pending_open: false,
            last_outer: None,
            spawn_pos,
            _tray: None,
            flags: Arc::new(Mutex::new(Flags {
                hidden: false,
                topmost: true,
                click_through: false,
            })),
        };

        // System tray: double-click shows/hides this pin, right-click menu.
        if let Some(icon) = tray_icon_from_ico() {
            let (menu, ids) = build_popup();
            let mut tooltip = String::from("PinPicture");
            if let Some(f) = app.file.as_ref().and_then(|f| f.file_name()) {
                tooltip = format!("PinPicture — {}", f.to_string_lossy());
            }
            let attrs = TrayIconAttributes {
                tooltip: Some(tooltip),
                icon: Some(icon),
                menu: Some(Box::new(menu)),
                menu_on_left_click: false,
                ..Default::default()
            };
            match TrayIcon::new(attrs) {
                Ok(t) => app._tray = Some(t),
                Err(e) => eprintln!("tray icon failed: {e}"),
            }

            // Dedicated polling thread so tray clicks work while the window is
            // hidden (a hidden window may not repaint, so ui() cannot poll).
            let ctx = ctx.clone();
            let flags = app.flags.clone();
            std::thread::spawn(move || loop {
                let mut busy = false;
                while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                    match ev {
                        TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                            // double-click = bring the pin back on top. "Minimize
                            // to tray" never hides the picture - it only stops the
                            // floating-on-top state; this re-pins it.
                            {
                                let mut f = flags.lock().unwrap();
                                f.hidden = false;
                                f.topmost = true;
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        _ => {}
                    }
                    busy = true;
                }
                while let Ok(me) = MenuEvent::receiver().try_recv() {
                    let mut f = flags.lock().unwrap();
                    if me.id == ids.exit {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else if me.id == ids.showhide {
                        f.hidden = !f.hidden;
                        let h = f.hidden;
                        drop(f);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!h));
                    } else if me.id == ids.topmost {
                        f.topmost = !f.topmost;
                    } else if me.id == ids.click {
                        f.click_through = !f.click_through;
                    }
                    busy = true;
                }
                if busy {
                    ctx.request_repaint();
                }
                std::thread::sleep(Duration::from_millis(100));
            });
        }
        app
    }

    fn bar_open(&self) -> bool {
        // no picture yet -> bar is the way in; with a picture the bar is hidden
        // unless the user toggled it back (right-click menu / double-click)
        self.image.is_none() || self.bar_visible
    }

    fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(0.05, 40.0);
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let Some(files) = rfd::FileDialog::new()
            .add_filter(
                "Picture files",
                &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"],
            )
            .pick_files()
        else {
            return;
        };
        if files.is_empty() {
            return;
        }
        self.file = Some(files[0].clone());
        self.image = load_pin_image(ctx, &files[0]);
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.rot = [0.0, 0.0, 0.0];
        self.flip_h = false;
        self.flip_v = false;
        let name = files[0]
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "picture".to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("PinPicture — {name}")));
        if let Some(t) = self._tray.as_mut() {
            let _ = t.set_tooltip(Some(format!("PinPicture — {name}")));
        }
        // every extra selected picture opens its own pin window (own process)
        let (px, py) = self.last_outer.map(|p| (p.x, p.y)).unwrap_or(self.spawn_pos);
        let Ok(exe) = std::env::current_exe() else { return };
        for (i, extra) in files[1..].iter().enumerate() {
            let _ = std::process::Command::new(&exe)
                .arg("--file")
                .arg(extra)
                .arg("--x")
                .arg((px + 32.0 * (i + 1) as f32).to_string())
                .arg("--y")
                .arg((py + 32.0 * (i + 1) as f32).to_string())
                .spawn();
        }
    }
}

#[cfg(windows)]
mod win_taskbar {
    //! A pinned picture is a desktop item, not an app window: give the
    //! top-level window WS_EX_TOOLWINDOW so it never shows a taskbar button.
    //! The system-tray icon is its only presence in the taskbar area.
    use std::ffi::c_void;
    type HWND = *mut c_void;
    extern "system" {
        fn EnumWindows(lpEnumProc: extern "system" fn(HWND, isize) -> i32, lParam: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdwProcessId: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: HWND) -> i32;
        fn GetWindowTextLengthW(hwnd: HWND) -> i32;
        fn GetWindowLongPtrW(hwnd: HWND, nIndex: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
        fn SetWindowPos(hwnd: HWND, insert_after: HWND, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    extern "system" fn each(hwnd: HWND, _lparam: isize) -> i32 {
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let vis = unsafe { IsWindowVisible(hwnd) };
        // NOTE: this process has more than one visible top-level window — the
        // tray-icon crate's hidden helper window (untitled) is one too, and it
        // races through EnumWindows first. Only restyle a window with a title.
        let title = unsafe { GetWindowTextLengthW(hwnd) };
        if pid != std::process::id() || vis == 0 || title == 0 {
            return 1;
        }
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if (ex & WS_EX_TOOLWINDOW) == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW);
                SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0,
                    SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_FRAMECHANGED);
            }
        }
        0 // stop: the titled top-level window of this (one-window) process
    }

    /// Returns true once the style has been applied (call every frame until so).
    pub fn hide_from_taskbar() -> bool {
        let r = unsafe { EnumWindows(each, 0) };
        r == 0
    }
}
#[cfg(not(windows))]
mod win_taskbar {
    pub fn hide_from_taskbar() -> bool {
        true
    }
}

impl eframe::App for PinApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // sync window flags (tray thread is authoritative)
        {
            let f = self.flags.lock().unwrap();
            self.topmost = f.topmost;
            self.click_through = f.click_through;
        }

        // restore click response when the window regains focus (like the
        // original: alt-tab back to it to make it clickable again)
        let focused = ctx.input(|i| i.focused);
        if focused && !self.prev_focused && self.click_through {
            self.flags.lock().unwrap().click_through = false;
            self.click_through = false;
        }
        self.prev_focused = focused;

        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.click_through));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.topmost {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        self.last_outer = ctx.input(|i| i.viewport().outer_rect.map(|r| r.min));

        // a pin has no taskbar button: only the tray icon (set once the
        // winit window exists; EnumWindows returns false = applied)
        if !self.taskbar_done {
            self.taskbar_done = win_taskbar::hide_from_taskbar();
        }

        // idle poll keeps tray responsive
        ctx.request_repaint_after(Duration::from_millis(400));

        // advance gif animation
        if let Some(im) = self.image.as_mut() {
            if im.frames.len() > 1 {
                let now = ctx.input(|i| i.time);
                if now >= im.next_at {
                    im.cur = (im.cur + 1) % im.frames.len();
                    im.tex.set(im.frames[im.cur].img.clone(), TextureOptions::LINEAR);
                    im.next_at = now + im.current_dur().as_secs_f64();
                }
                ctx.request_repaint_after(im.current_dur().min(Duration::from_millis(400)));
            }
        }

        // ---- content: the pinned picture ----
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::TRANSPARENT))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let mut resp = ui.interact(rect, ui.id().with("content"), Sense::click());
                if resp.double_clicked() {
                    self.bar_visible = !self.bar_visible;
                }
                let zd = ui.input(|i| i.zoom_delta());
                if (zd - 1.0).abs() > f32::EPSILON && resp.hovered() {
                    let m = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
                    let old = self.zoom;
                    let new = (old * zd).clamp(0.05, 40.0);
                    let rel = m - rect.center() - self.pan;
                    self.pan = self.pan + rel - rel * (new / old);
                    self.zoom = new;
                }
                if self.image.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("No picture pinned — Open or right-click for menu")
                                .size(13.0)
                                .color(Color32::from_white_alpha(200)),
                        )
                    });
                }
                self.draw_picture(ui, rect);
                // right-click anywhere on the picture = the whole control menu
                resp.context_menu(|ui| {
                    self.context_menu(ui);
                });
            });

        if std::mem::take(&mut self.pending_open) {
            self.open_dialog(&ctx);
        }

        let win_w = ctx.input(|i| i.viewport().inner_rect.map(|r| r.width())).unwrap_or(640.0);

        // ---- title bar: only when no picture loaded, or explicitly toggled on
        if self.bar_open() {
            let bar_w = win_w - 20.0;
            egui::Area::new(egui::Id::new("pinbar"))
                .anchor(
                    if self.bar_top { Align2::CENTER_TOP } else { Align2::CENTER_BOTTOM },
                    [0.0, if self.bar_top { 6.0 } else { -6.0 }],
                )
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(24, 24, 24, 150))
                        .corner_radius(14.0)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(bar_w, BAR_H));
                            // drag rect goes BEHIND the buttons: dragging empty bar
                            // space moves the window, buttons keep their clicks
                            let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(bar_w, BAR_H));
                            let drag = ui.interact(rect, ui.id().with("bardrag"), Sense::drag());
                            if drag.dragged() {
                                if let Some(op) =
                                    ctx.input(|i| i.viewport().outer_rect.map(|r| r.min))
                                {
                                    let d = drag.drag_delta();
                                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                                        Pos2 { x: op.x + d.x, y: op.y + d.y },
                                    ));
                                }
                            }
                            self.draw_bar(ui, bar_w);
                        });
                });
        }

        // ---- resize grip (bottom-right, like the original red dot) ----
        egui::Area::new(egui::Id::new("pingrip"))
            .anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0])
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                let (grect, gresp) = ui.allocate_at_least(Vec2::splat(18.0), Sense::drag());
                if gresp.dragged() {
                    let d = gresp.drag_delta();
                    if let Some(is) = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size())) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2 {
                            x: (is.x + d.x).max(64.0),
                            y: (is.y + d.y).max(48.0),
                        }));
                    }
                }
                ui.painter().circle_filled(
                    grect.center(),
                    7.0,
                    Color32::from_rgba_unmultiplied(
                        223, 41, 41,
                        if self.bar_open() { 160 } else { 90 },
                    ),
                );
            });

        // ---- settings panel (opened from the button bar) ----
        if self.settings_open && self.bar_open() {
            egui::Area::new(egui::Id::new("pinsettings"))
                .anchor(
                    if self.bar_top { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP },
                    [10.0, if self.bar_top { -48.0 } else { 48.0 }],
                )
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(24, 24, 24, 180))
                        .corner_radius(12.0)
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(150.0);
                            ui.label("Rotation");
                            for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
                                let r = ui
                                    .add(egui::Slider::new(&mut self.rot[i], -180.0..=180.0).text(*name));
                                if r.double_clicked() {
                                    self.rot[i] = 0.0;
                                }
                            }
                            ui.horizontal(|ui| {
                                if ui.selectable_label(self.flip_h, "Flip H").clicked() {
                                    self.flip_h = !self.flip_h;
                                }
                                if ui.selectable_label(self.flip_v, "Flip V").clicked() {
                                    self.flip_v = !self.flip_v;
                                }
                            });
                            if ui.checkbox(&mut self.autostart, "Auto Start").changed() {
                                self.autostart_err = autostart_set(self.autostart).err();
                            }
                            if let Some(err) = &self.autostart_err {
                                ui.colored_label(Color32::LIGHT_RED, err);
                            }
                        });
                });
        }

        // ---- about ----
        if self.about_open {
            let resp = egui::Area::new(egui::Id::new("pinabout"))
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(33, 32, 32, 200))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("PinPicture v 1.4 (Rust port)").size(18.0));
                            ui.label("Pin any picture to the desktop");
                            ui.label("Original: vhanla — https://github.com/vhanla/PinPicture");
                            ui.label("Right-click picture for the menu · drag with the bar · Ctrl+wheel zoom");
                        });
                })
                .response;
            if resp.clicked() {
                self.about_open = false;
            }
        }
    }
}

impl PinApp {
    /// Right-click menu over the picture: everything the bar can do.
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Open Picture(s)…").clicked() {
            self.pending_open = true;
            ui.close();
        }
        ui.menu_button("Zoom", |ui| {
            if ui.button("Zoom in").clicked() {
                let z = self.zoom * 1.25;
                self.set_zoom(z);
                ui.close();
            }
            if ui.button("Zoom out").clicked() {
                let z = self.zoom / 1.25;
                self.set_zoom(z);
                ui.close();
            }
            if ui.button("Reset").clicked() {
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
                ui.close();
            }
            ui.label(format!("{:.0}%", self.zoom * 100.0));
        });
        ui.menu_button("Rotate", |ui| {
            for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
                let r = ui.add(egui::Slider::new(&mut self.rot[i], -180.0..=180.0).text(*name));
                if r.double_clicked() {
                    self.rot[i] = 0.0;
                }
            }
            ui.horizontal(|ui| {
                if ui.selectable_label(self.flip_h, "Flip H").clicked() {
                    self.flip_h = !self.flip_h;
                }
                if ui.selectable_label(self.flip_v, "Flip V").clicked() {
                    self.flip_v = !self.flip_v;
                }
            });
        });
        if ui
            .add(egui::Slider::new(&mut self.opacity, 0.05..=1.0).text("Alpha"))
            .double_clicked()
        {
            self.opacity = 1.0;
        }
        if ui.checkbox(&mut self.topmost, "Topmost").changed() {
            self.flags.lock().unwrap().topmost = self.topmost;
        }
        if ui.checkbox(&mut self.click_through, "Click-through").changed() {
            self.flags.lock().unwrap().click_through = self.click_through;
        }
        ui.checkbox(&mut self.bar_visible, "Button bar");
        if ui.checkbox(&mut self.autostart, "Auto Start on logon").changed() {
            self.autostart_err = autostart_set(self.autostart).err();
        }
        if let Some(err) = &self.autostart_err {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
        ui.separator();
        if ui.button("Minimize to Tray").clicked() {
            // app retires to the tray; the picture stays on the desktop but
            // stops floating on top (double-click the tray icon to re-pin)
            self.flags.lock().unwrap().topmost = false;
            ui.close();
        }
        if ui.button("About…").clicked() {
            self.about_open = true;
            ui.close();
        }
        if ui.button("Close PinPicture").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            ui.close();
        }
    }

    fn draw_bar(&mut self, ui: &mut egui::Ui, bar_w: f32) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            if glyph_button(ui, G_BARFLIP, "Switch bar top/bottom").clicked() {
                self.bar_top = !self.bar_top;
            }
            if glyph_button(ui, G_INFO, "About this tool").clicked() {
                self.about_open = !self.about_open;
            }
            ui.add_space(8.0);
            let a = ui.add_sized(
                [130.0, 24.0],
                egui::Slider::new(&mut self.opacity, 0.05..=1.0).show_value(false),
            );
            if a.on_hover_text("Transparency level (double-click = 100%)").double_clicked() {
                self.opacity = 1.0;
            }
            // push the right-hand button group to the right end of the bar
            let used = ui.min_rect().width();
            let right_n = 6.0;
            let pad = (bar_w - 16.0 - used - right_n * 34.0).max(0.0);
            ui.add_space(pad);
            if glyph_button(ui, G_OPEN, "Open Picture(s)").clicked() {
                self.pending_open = true;
            }
            if glyph_button(ui, G_SETTINGS, "Rotation & settings").clicked() {
                self.settings_open = !self.settings_open;
            }
            if glyph_button(ui, G_CLICK, "Click-through (alt-tab back to restore clicks)").clicked() {
                self.click_through = !self.click_through;
                self.flags.lock().unwrap().click_through = self.click_through;
            }
            let pin_hint = if self.topmost { "TopMost: on" } else { "TopMost: off" };
            if glyph_button(ui, if self.topmost { G_PIN_ON } else { G_PIN_OFF }, pin_hint).clicked() {
                self.topmost = !self.topmost;
                self.flags.lock().unwrap().topmost = self.topmost;
            }
            if glyph_button(ui, G_TRAY, "Minimize to Tray (picture stays, stops floating)").clicked() {
                self.flags.lock().unwrap().topmost = false;
            }
            if glyph_button(ui, G_CLOSE, "Close PinPicture").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn draw_picture(&self, ui: &mut egui::Ui, rect: Rect) {
        let Some(im) = self.image.as_ref() else { return };
        let c = rect.center() + self.pan;
        let hw = (rect.width() * self.zoom / 2.0).max(1.0);
        let hh = (rect.height() * self.zoom / 2.0).max(1.0);
        let (rx, ry, rz) = (
            self.rot[0].to_radians(),
            self.rot[1].to_radians(),
            self.rot[2].to_radians(),
        );
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let (sz, cz) = rz.sin_cos();

        const N: usize = 10; // warp grid resolution
        let mut mesh = egui::Mesh::default();
        mesh.texture_id = im.tex.id();
        let tint = Color32::from_white_alpha((self.opacity * 255.0) as u8);
        for gy in 0..=N {
            for gx in 0..=N {
                let fx = gx as f32 / N as f32;
                let fy = gy as f32 / N as f32;
                let x = (fx * 2.0 - 1.0) * hw;
                let y = (fy * 2.0 - 1.0) * hh;
                // rotate around X (y -> y', z)
                let y1 = y * cx;
                let z1 = y * sx;
                // rotate around Y (x, z -> x', z')
                let x2 = x * cy + z1 * sy;
                let z2 = -x * sy + z1 * cy;
                // rotate around Z
                let x3 = x2 * cz - y1 * sz;
                let y3 = x2 * sz + y1 * cz;
                let f = PERSPECTIVE / (PERSPECTIVE - z2).max(80.0);
                // egui 0.37 wants NORMALIZED uvs (0..1), not texture pixels
                let mut u = fx;
                let mut v = fy;
                if self.flip_h {
                    u = 1.0 - u;
                }
                if self.flip_v {
                    v = 1.0 - v;
                }
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2 { x: c.x + x3 * f, y: c.y + y3 * f },
                    uv: egui::pos2(u, v),
                    color: tint,
                });
            }
        }
        for gy in 0..N {
            for gx in 0..N {
                let a = (gy * (N + 1) + gx) as u32;
                let b = a + 1;
                let d = a + (N + 1) as u32;
                let e = d + 1;
                mesh.indices.extend_from_slice(&[a, b, d, b, e, d]);
            }
        }
        ui.painter().add(mesh);
    }
}

fn glyph_button(ui: &mut egui::Ui, g: char, hint: &str) -> egui::Response {
    let resp = ui.add(
        egui::Button::new(egui::RichText::new(String::from(g)).size(16.0))
            .min_size(Vec2::new(30.0, 30.0)),
    );
    resp.on_hover_text(hint)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let title = args
        .file
        .as_ref()
        .and_then(|f| f.file_name().map(|n| format!("PinPicture — {}", n.to_string_lossy())))
        .unwrap_or_else(|| "PinPicture".to_owned());

    let mut vb = egui::ViewportBuilder::default()
        .with_inner_size([640.0, 480.0])
        .with_transparent(true)
        .with_decorations(false)
        .with_window_level(egui::WindowLevel::AlwaysOnTop)
        .with_title(title);
    if let (Some(x), Some(y)) = (args.x, args.y) {
        vb = vb.with_position(Pos2 { x, y });
    }

    let opts = eframe::NativeOptions {
        viewport: vb,
        ..Default::default()
    };
    eframe::run_native(
        "PinPicture",
        opts,
        Box::new(move |cc| Ok(Box::new(PinApp::new(cc, args)))),
    )
    .map_err(|e| format!("eframe error: {e}"))?;
    Ok(())
}
