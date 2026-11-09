#![allow(unsafe_op_in_unsafe_fn)]
// Rust 1.98's lint also inspects the standard macro's internal lazy initializer.
#![allow(clippy::missing_const_for_thread_local)]

mod debug;
mod desktop;
mod music;
#[cfg(test)]
mod panel_tests;
mod settings;
mod tray;
mod ui;

use crate::{
    catalog::Catalog,
    player::Player,
    poses::{Pose, Poses},
    render::{self, Bounds},
    wander::{DEFAULT_DISTANCE_BIAS, DEFAULT_WALK_CHANCE, Wander},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    mem::size_of,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::NIN_SELECT,
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const SEARCH: usize = 100;
const KIND: usize = 101;
const CATEGORY: usize = 102;
const LIST: usize = 103;
const PLAY: usize = 200;
const RESTART: usize = 201;
const PREV: usize = 202;
const NEXT: usize = 203;
const LOOP: usize = 204;
const SPEED: usize = 205;
const SCALE: usize = 207;
const BG: usize = 208;
const TIMELINE: usize = 209;
const DETAILS: usize = 211;
const IDLE: usize = 212;
const DETAILS_CLOSE: usize = 213;
const CLEAR_FILTERS: usize = 214;
const MENU_DEBUG: u32 = 400;
const MENU_EXIT: u32 = 401;
const MENU_SETTINGS: u32 = 402;
const WALK_CHANCE: usize = 500;
const DISTANCE_BIAS: usize = 506;
const DANCE_TO_MUSIC: usize = 507;
const SETTINGS_HEIGHT: i32 = 480;
const SHOW_DEBUG: u32 = WM_APP + 1;
const RATES: [f64; 7] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0];

thread_local! { static APP:RefCell<Option<App>>=const{RefCell::new(None)}; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn text(hwnd: HWND, s: &str) {
    let s = wide(s);
    let _ = SetWindowTextW(hwnd, PCWSTR(s.as_ptr()));
}
unsafe fn send(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    SendMessageW(hwnd, msg, Some(WPARAM(wp)), Some(LPARAM(lp))).0
}
unsafe fn window_text(hwnd: HWND) -> String {
    let mut buf = vec![0u16; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
    let len = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len as usize])
}

#[derive(Serialize, Deserialize)]
struct Settings {
    anchor_x: i32,
    anchor_y: i32,
    scale: u32,
    #[serde(default = "default_walk_chance")]
    walk_probability: u8,
    #[serde(default = "default_distance_bias")]
    walk_distance_bias: u8,
    #[serde(default = "default_dance_to_music")]
    dance_to_music: bool,
}
fn default_walk_chance() -> u8 {
    DEFAULT_WALK_CHANCE
}
fn default_distance_bias() -> u8 {
    DEFAULT_DISTANCE_BIAS
}
fn default_dance_to_music() -> bool {
    true
}
fn settings_path() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
        .join("AquaPet/settings.json")
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut c_void,
    width: i32,
    height: i32,
}
impl Surface {
    unsafe fn new(width: i32, height: i32) -> windows::core::Result<Self> {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return Err(windows::core::Error::from_thread());
        }
        let info = bitmap_info(width, height);
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) => b,
            Err(e) => {
                let _ = DeleteDC(dc);
                return Err(e);
            }
        };
        let previous = SelectObject(dc, bitmap.into());
        Ok(Self {
            dc,
            bitmap,
            previous,
            bits,
            width,
            height,
        })
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Drag {
    start: POINT,
    anchor: POINT,
    moved: bool,
}

struct App {
    catalog: Catalog,
    player: Player,
    pet: HWND,
    debug: HWND,
    settings: HWND,
    tray: Option<tray::TrayIcon>,
    search: HWND,
    fonts: ui::Fonts,
    settings_fonts: ui::Fonts,
    debug_ui: ui::State,
    settings_ui: ui::State,
    debug_regions: HashMap<usize, RECT>,
    settings_regions: HashMap<usize, settings::Row>,
    kind_filter: usize,
    category_filter: usize,
    details_open: bool,
    details_scroll: i32,
    details_height: i32,
    visible: Vec<usize>,
    choices: HashMap<usize, Vec<String>>,
    bounds: Bounds,
    motion_bounds: Bounds,
    desktop: desktop::Desktop,
    music: Option<music::Monitor>,
    dance_to_music: bool,
    poses: Poses,
    auto_pose: Pose,
    wander: Wander,
    scale: u32,
    effective_scale: u32,
    anchor: POINT,
    drag: Option<Drag>,
    surface: Option<Surface>,
    last_clock: Instant,
    timer_ms: u32,
    last_status: Instant,
    last_frame: Option<(usize, usize, bool, u32)>,
    last_position: Option<(i32, i32)>,
    preview: Option<(BITMAPINFO, Vec<u8>)>,
    preview_rect: RECT,
    preview_dirty: bool,
    bg: usize,
    debug_open: bool,
    closing: bool,
    auto_exit: Option<Instant>,
    smoke: bool,
}

impl App {
    fn new(catalog: Catalog, pet: HWND, debug: HWND, settings: HWND, smoke: bool) -> Self {
        let idle = catalog.idle_index();
        let bounds = render::clip_bounds(&catalog, &catalog.clips[idle], false);
        let poses = Poses::new(&catalog);
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ std::process::id() as u64;
        let player = Player::new(idle, &catalog.clips[idle]);
        let wander = Wander::new(seed, poses.durations(&catalog));
        Self {
            catalog,
            player,
            pet,
            debug,
            settings,
            tray: None,
            search: HWND::default(),
            fonts: ui::Fonts::default(),
            settings_fonts: ui::Fonts::default(),
            debug_ui: ui::State {
                focus: Some(LIST),
                ..Default::default()
            },
            settings_ui: ui::State {
                focus: Some(WALK_CHANCE),
                ..Default::default()
            },
            debug_regions: HashMap::new(),
            settings_regions: HashMap::new(),
            kind_filter: 0,
            category_filter: 0,
            details_open: false,
            details_scroll: 0,
            details_height: 0,
            visible: vec![],
            choices: HashMap::new(),
            bounds,
            motion_bounds: bounds,
            desktop: desktop::Desktop::new(&[pet, debug, settings]),
            music: None,
            dance_to_music: default_dance_to_music(),
            poses,
            auto_pose: Pose::Idle,
            wander,
            scale: 4,
            effective_scale: 4,
            anchor: POINT { x: 0, y: 0 },
            drag: None,
            surface: None,
            last_clock: Instant::now(),
            timer_ms: 0,
            last_status: Instant::now(),
            last_frame: None,
            last_position: None,
            preview: None,
            preview_rect: RECT::default(),
            preview_dirty: true,
            bg: 0,
            debug_open: false,
            closing: false,
            auto_exit: if smoke {
                Some(Instant::now() + Duration::from_secs(3))
            } else {
                None
            },
            smoke,
        }
    }
    unsafe fn work_area(&self) -> RECT {
        Self::work_area_at(self.anchor)
    }
    unsafe fn work_area_at(point: POINT) -> RECT {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            info.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            }
        }
    }
    unsafe fn work_bounds_at(point: POINT) -> Bounds {
        let area = Self::work_area_at(point);
        Bounds {
            left: area.left as f64,
            top: area.top as f64,
            right: area.right as f64,
            bottom: area.bottom as f64,
        }
    }
    unsafe fn fit(&mut self, clap_on_landing: bool) {
        let work = Self::work_bounds_at(self.anchor);
        if !self.debug_open {
            self.effective_scale =
                crate::placement::fit_scale(self.scale, self.poses.body(1), work);
            let s = self.effective_scale as f64;
            let (fx, fy) = self.poses.get(Pose::Idle).foot(0);
            let limits = self
                .desktop
                .ledges(work, self.poses.body(self.effective_scale))
                .limits;
            let foot = (
                (self.anchor.x as f64 + fx * s).clamp(limits.left, limits.right),
                (self.anchor.y as f64 + fy * s).clamp(limits.top, limits.bottom),
            );
            self.wander.drop_at(foot, clap_on_landing);
            self.sync_anchor();
            return;
        }
        let ((x, y), scale) = crate::placement::fit(
            (self.anchor.x, self.anchor.y),
            self.scale,
            self.motion_bounds,
            work,
        );
        self.anchor = POINT { x, y };
        self.effective_scale = scale;
    }
    fn sync_anchor(&mut self) {
        // Saved positions and Debug use the original idle sprite origin.
        let (fx, fy) = self.poses.get(Pose::Idle).foot(0);
        let scale = self.effective_scale as f64;
        self.anchor = POINT {
            x: (self.wander.foot.0 - fx * scale).round() as i32,
            y: (self.wander.foot.1 - fy * scale).round() as i32,
        };
    }
    unsafe fn restore_settings(&mut self) {
        let settings = std::fs::read(settings_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok());
        if let Some(s) = settings {
            self.scale = s.scale.clamp(1, 8);
            self.wander.set_probability(s.walk_probability);
            self.wander.set_distance_bias(s.walk_distance_bias);
            self.dance_to_music = s.dance_to_music;
            self.anchor = POINT {
                x: s.anchor_x,
                y: s.anchor_y,
            };
        } else {
            let area = self.work_area();
            self.anchor = POINT {
                x: area.right - 32 - (self.bounds.right * 4.0) as i32,
                y: area.bottom - 32 - (self.bounds.bottom * 4.0) as i32,
            };
        }
        self.fit(false);
    }
    fn save_settings(&self) {
        if self.smoke {
            return;
        }
        let path = settings_path();
        let data = Settings {
            anchor_x: self.anchor.x,
            anchor_y: self.anchor.y,
            scale: self.scale,
            walk_probability: self.wander.probability(),
            walk_distance_bias: self.wander.distance_bias(),
            dance_to_music: self.dance_to_music,
        };
        if let (Some(parent), Ok(bytes)) = (path.parent(), serde_json::to_vec_pretty(&data))
            && std::fs::create_dir_all(parent).is_ok()
        {
            let _ = std::fs::write(path, bytes);
        }
    }
    fn select(&mut self, index: usize) {
        self.player.select(index, &self.catalog.clips[index]);
        self.bounds = render::clip_bounds(&self.catalog, &self.catalog.clips[index], false);
        self.motion_bounds = render::clip_bounds(&self.catalog, &self.catalog.clips[index], true);
        self.last_frame = None;
        self.preview_dirty = true;
        unsafe {
            self.fit(false);
            if self.debug_open {
                self.sync_selection();
            }
        }
        self.last_clock = Instant::now();
    }
    fn show_pose(&mut self, pose: Pose) {
        let clip = self.poses.get(pose);
        if self.auto_pose != pose {
            self.last_frame = None;
        }
        self.auto_pose = pose;
        if self.player.selected != clip.index {
            self.player
                .select(clip.index, &self.catalog.clips[clip.index]);
            self.last_frame = None;
        }
        self.bounds = clip.bounds;
    }
    unsafe fn show_debug(&mut self) -> windows::core::Result<()> {
        if !self.debug_open {
            self.catalog
                .load_composites()
                .map_err(|error| windows::core::Error::new(E_FAIL, error))?;
            if self.search.is_invalid() {
                if let Err(error) = self.create_debug_controls() {
                    self.catalog.unload_composites();
                    return Err(error);
                }
            } else {
                self.filter();
            }
            self.debug_open = true;
            self.select(self.catalog.idle_index());
        }
        self.preview_dirty = true;
        let _ = ShowWindow(self.debug, SW_RESTORE);
        let _ = SetForegroundWindow(self.debug);
        if self.debug_ui.focus == Some(SEARCH) {
            let _ = SetFocus(Some(self.search));
        } else {
            let _ = SetFocus(Some(self.debug));
        }
        self.layout();
        self.update()
    }
    unsafe fn close_debug(&mut self) {
        self.debug_open = false;
        self.details_open = false;
        self.debug_ui.popup = None;
        self.debug_ui.active = None;
        let _ = ShowWindow(self.debug, SW_HIDE);
        self.show_pose(Pose::Idle);
        self.visible.clear();
        self.preview = None;
        self.catalog.unload_composites();
        self.fit(false);
        self.save_settings();
    }
    unsafe fn exit(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.save_settings();
        self.tray.take();
        let _ = KillTimer(Some(self.pet), 1);
        let _ = DestroyWindow(self.debug);
        let _ = DestroyWindow(self.settings);
        let _ = DestroyWindow(self.pet);
        PostQuitMessage(0);
    }
    unsafe fn menu(&mut self, point: Option<POINT>) {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS as usize, w!("设置"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_DEBUG as usize, w!("Debug · 动作预览"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, w!("关闭"));
        let point = point.unwrap_or_else(|| {
            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);
            point
        });
        let _ = SetForegroundWindow(self.pet);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            self.pet,
            None,
        )
        .0 as u32;
        // Keep subsequent tray menus open until the user selects or dismisses them.
        let _ = PostMessageW(Some(self.pet), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        self.last_clock = Instant::now();
        if command == MENU_SETTINGS {
            self.show_settings();
        } else if command == MENU_DEBUG {
            if let Err(error) = self.show_debug() {
                show_error(&format!("无法打开 Debug：{error}"));
            }
        } else if command == MENU_EXIT {
            self.exit();
        }
    }
    unsafe fn update(&mut self) -> windows::core::Result<()> {
        if self
            .auto_exit
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.exit();
            return Ok(());
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_clock).as_secs_f64();
        self.last_clock = now;
        let was_playing = self.player.playing;
        // Resuming from sleep or a suspended process should not jump across an entire scene.
        if dt < 2.0 {
            if !self.debug_open && self.drag.is_none() {
                let carried = self.desktop.refresh(now, self.wander.riding());
                let (x, y) = carried.unwrap_or(self.wander.foot);
                let work = Self::work_bounds_at(POINT {
                    x: x.round() as i32,
                    y: y.round() as i32 - 1,
                });
                self.effective_scale =
                    crate::placement::fit_scale(self.scale, self.poses.body(1), work);
                let body = self.poses.body(self.effective_scale);
                let music_playing = self.dance_to_music
                    && !self.smoke
                    && self
                        .music
                        .get_or_insert_with(music::Monitor::new)
                        .playing(now);
                self.wander.set_music_playing(music_playing);
                let (approached, idle) = self.desktop.activity((x, y), body);
                self.wander.observe(approached, idle, dt);
                let world = self.desktop.ledges(work, body);
                self.wander
                    .advance(dt, self.catalog.game_fps, self.effective_scale, world);
                self.sync_anchor();
                self.show_pose(self.wander.pose());
                let clip = &self.catalog.clips[self.player.selected];
                self.player.tick =
                    (self.wander.animation_seconds() * self.catalog.game_fps) % clip.duration();
            } else if self.debug_open || self.drag.as_ref().is_some_and(|drag| drag.moved) {
                let clip = &self.catalog.clips[self.player.selected];
                self.player.advance(dt, self.catalog.game_fps, clip);
            }
        }
        let clip = &self.catalog.clips[self.player.selected];
        let index = self.player.index(clip);
        let key = (
            self.player.selected,
            index,
            self.player.finished,
            self.effective_scale,
        );
        let changed = self.last_frame != Some(key);
        if changed {
            let rendered;
            let (frame, scale) = if self.debug_open {
                rendered = render::render(
                    &self.catalog,
                    &clip.steps[index],
                    self.bounds,
                    self.effective_scale as f64,
                    false,
                );
                (&rendered, 1)
            } else {
                (
                    self.poses.get(self.auto_pose).frame(index),
                    self.effective_scale as usize,
                )
            };
            let width = (frame.width * scale) as i32;
            let height = (frame.height * scale) as i32;
            if self
                .surface
                .as_ref()
                .is_none_or(|s| s.width != width || s.height != height)
            {
                self.surface = Some(Surface::new(width, height)?);
            }
            let surface = self.surface.as_mut().unwrap();
            let pixels = std::slice::from_raw_parts_mut(
                surface.bits.cast(),
                width as usize * height as usize,
            );
            frame.write_bgra(pixels, scale);
            self.preview_dirty = true;
        }
        if let Some(surface) = &self.surface {
            let (rx, ry) = if self.debug_open {
                self.player.root(clip)
            } else {
                let (ix, iy) = self.poses.get(Pose::Idle).foot(0);
                let (fx, fy) = self.poses.get(self.auto_pose).foot(index);
                (ix - fx, iy - fy)
            };
            let s = self.effective_scale as f64;
            let dst = POINT {
                x: self.anchor.x + ((self.bounds.left + rx) * s).round() as i32,
                y: self.anchor.y + ((self.bounds.top + ry) * s).round() as i32,
            };
            if changed {
                let size = SIZE {
                    cx: surface.width,
                    cy: surface.height,
                };
                let src = POINT::default();
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                UpdateLayeredWindow(
                    self.pet,
                    None,
                    Some(&dst),
                    Some(&size),
                    Some(surface.dc),
                    Some(&src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                )?;
                self.last_frame = Some(key);
            } else if self.last_position != Some((dst.x, dst.y)) {
                UpdateLayeredWindow(
                    self.pet,
                    None,
                    Some(&dst),
                    None,
                    None,
                    None,
                    COLORREF(0),
                    None,
                    UPDATE_LAYERED_WINDOW_FLAGS(0),
                )?;
            }
            self.last_position = Some((dst.x, dst.y));
        }
        // Restore the window's order even when the animation hasn't changed.
        if GetWindowLongPtrW(self.pet, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 == 0 {
            SetWindowPos(
                self.pet,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )?;
        }
        let preview_visible =
            self.debug_open && !self.details_open && !IsIconic(self.debug).as_bool();
        if preview_visible && self.preview_dirty {
            let rw = (self.preview_rect.right - self.preview_rect.left - 24).max(1) as f64;
            let rh = (self.preview_rect.bottom - self.preview_rect.top - 24).max(1) as f64;
            let fit = (rw / self.motion_bounds.width())
                .min(rh / self.motion_bounds.height())
                .min(8.0);
            let scale = if fit >= 1.0 {
                fit.floor()
            } else {
                fit.max(0.05)
            };
            let mut step = clip.steps[index].clone();
            if self.player.finished {
                step.root_x = 0.0;
                step.root_y = 0.0;
            }
            let frame =
                render::render_preview(&self.catalog, &step, self.motion_bounds, scale, self.bg);
            self.preview = Some((
                bitmap_info(frame.width as i32, frame.height as i32),
                frame.on_background(self.bg),
            ));
            self.preview_dirty = false;
            let _ = InvalidateRect(Some(self.debug), Some(&self.preview_rect), false);
        }
        if preview_visible
            && was_playing
            && (!self.player.playing
                || now.duration_since(self.last_status) >= Duration::from_millis(100))
        {
            self.invalidate_playback();
            self.last_status = now;
        }
        let interval = if self.debug_open
            || self.drag.is_some()
            || self.wander.riding().is_some()
            || matches!(
                self.auto_pose,
                Pose::Left | Pose::Right | Pose::Rise | Pose::Fall | Pose::Roll | Pose::Flattery
            ) {
            16
        } else {
            // A stationary pet still needs to notice nearby input between animation frames.
            let ticks_left = clip.step_start(index) + clip.steps[index].ticks - self.player.tick;
            (ticks_left * 1000.0 / self.catalog.game_fps)
                .ceil()
                .clamp(16.0, 100.0) as u32
        };
        if interval != self.timer_ms {
            if SetTimer(Some(self.pet), 1, interval, None) == 0 {
                return Err(windows::core::Error::from_thread());
            }
            self.timer_ms = interval;
        }
        Ok(())
    }
    unsafe fn handle(&mut self, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let debug = hwnd == self.debug;
        let pet = hwnd == self.pet;
        let settings = hwnd == self.settings;
        match msg {
            SHOW_DEBUG if pet => {
                if let Err(error) = self.show_debug() {
                    show_error(&format!("无法打开 Debug：{error}"));
                }
                Some(LRESULT(0))
            }
            WM_TIMER if pet => {
                if let Err(error) = self.update() {
                    show_error(&format!("绘制失败：{error}"));
                    self.exit();
                }
                Some(LRESULT(0))
            }
            WM_CONTEXTMENU if pet => {
                self.menu(None);
                Some(LRESULT(0))
            }
            tray::CALLBACK_MESSAGE if pet => {
                if matches!(
                    lp.0 as u32 & 0xffff,
                    NIN_SELECT | tray::NIN_KEYSELECT | WM_CONTEXTMENU
                ) {
                    self.menu(Some(POINT {
                        x: wp.0 as u16 as i16 as i32,
                        y: (wp.0 >> 16) as u16 as i16 as i32,
                    }));
                }
                Some(LRESULT(0))
            }
            _ if pet
                && self
                    .tray
                    .as_ref()
                    .is_some_and(|tray| msg == tray.taskbar_created) =>
            {
                if let Err(error) = self.tray.as_ref().unwrap().add() {
                    show_error(&format!("恢复托盘图标失败：{error}"));
                }
                Some(LRESULT(0))
            }
            WM_MOUSEACTIVATE if pet => Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_LBUTTONDOWN if pet => {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                self.drag = Some(Drag {
                    start: p,
                    anchor: self.anchor,
                    moved: false,
                });
                SetCapture(self.pet);
                let _ = self.update();
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE if pet => {
                if let Some(drag) = &mut self.drag {
                    let mut p = POINT::default();
                    let _ = GetCursorPos(&mut p);
                    drag.moved |= p != drag.start;
                    self.anchor = POINT {
                        x: drag.anchor.x + p.x - drag.start.x,
                        y: drag.anchor.y + p.y - drag.start.y,
                    };
                    if drag.moved && !self.debug_open {
                        self.show_pose(Pose::Sleeve);
                    }
                    let _ = self.update();
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP if pet => {
                if let Some(drag) = self.drag.take() {
                    let _ = ReleaseCapture();
                    if drag.moved {
                        self.fit(true);
                        self.save_settings();
                    } else if !self.debug_open {
                        self.wander.flatter();
                    }
                    self.last_clock = Instant::now();
                    let _ = self.update();
                }
                Some(LRESULT(0))
            }
            WM_CAPTURECHANGED if pet => {
                if let Some(drag) = self.drag.take() {
                    if drag.moved {
                        self.fit(false);
                        self.save_settings();
                    }
                    self.last_clock = Instant::now();
                    let _ = self.update();
                }
                Some(LRESULT(0))
            }
            WM_COMMAND if debug => {
                self.debug_command(wp);
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE | WM_MOUSELEAVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEWHEEL
                if debug =>
            {
                self.debug_mouse(msg, wp, lp);
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE | WM_MOUSELEAVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEWHEEL
                if settings =>
            {
                self.settings_mouse(msg, wp, lp);
                Some(LRESULT(0))
            }
            WM_KEYDOWN if debug => {
                self.debug_key(VIRTUAL_KEY(wp.0 as u16));
                Some(LRESULT(0))
            }
            WM_KEYDOWN if settings => {
                let key = VIRTUAL_KEY(wp.0 as u16);
                if key != VK_SPACE || lp.0 & (1 << 30) == 0 {
                    self.settings_key(key);
                }
                Some(LRESULT(0))
            }
            WM_CAPTURECHANGED if debug || settings => {
                self.debug_ui.active = None;
                self.settings_ui.active = None;
                Some(LRESULT(0))
            }
            WM_SIZE if debug => {
                self.layout();
                Some(LRESULT(0))
            }
            WM_SIZE if settings => {
                self.layout_settings();
                Some(LRESULT(0))
            }
            WM_PAINT if debug && self.debug_open => {
                ui::paint_window(hwnd, |dc, client| self.paint(dc, client));
                Some(LRESULT(0))
            }
            WM_PAINT if settings => {
                ui::paint_window(hwnd, |dc, client| self.paint_settings(dc, client));
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                if debug {
                    self.close_debug();
                } else if settings {
                    self.save_settings();
                    let _ = ShowWindow(self.settings, SW_HIDE);
                } else {
                    self.exit();
                }
                Some(LRESULT(0))
            }
            WM_DISPLAYCHANGE => {
                if self.debug_open {
                    self.fit(false);
                }
                self.last_frame = None;
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                if debug || settings {
                    let rect = &*(lp.0 as *const RECT);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                    if debug {
                        self.update_font();
                        self.layout();
                    } else {
                        self.update_settings_font();
                        self.layout_settings();
                    }
                } else {
                    if self.debug_open {
                        self.fit(false);
                    }
                    self.last_frame = None;
                }
                Some(LRESULT(0))
            }
            WM_GETMINMAXINFO if debug => {
                let info = &mut *(lp.0 as *mut MINMAXINFO);
                let dpi = GetDpiForWindow(hwnd).max(96) as i32;
                info.ptMinTrackSize = POINT {
                    x: 920 * dpi / 96,
                    y: 700 * dpi / 96,
                };
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if GetWindowLongPtrW(hwnd, GWLP_USERDATA) == 1
        && let Some(result) = ui::edit_color(msg, wp)
    {
        return result;
    }
    // Native controls can synchronously re-enter the parent while we change their values.
    // try_borrow_mut prevents overlapping Rust mutable references on those callbacks.
    let handled = APP.with(|slot| {
        slot.try_borrow_mut()
            .ok()
            .and_then(|mut app| app.as_mut().and_then(|app| app.handle(hwnd, msg, wp, lp)))
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp))
}

pub fn show_error(message: &str) {
    unsafe {
        let message = wide(message);
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            w!("AQUA"),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Reopening the executable keeps the existing pet and can reveal its hidden panel.
pub fn show_existing(debug: bool) -> bool {
    unsafe {
        if let Ok(pet) = FindWindowW(w!("AquaPetOverlay"), None) {
            if debug {
                let _ = PostMessageW(Some(pet), SHOW_DEBUG, WPARAM(0), LPARAM(0));
            }
            true
        } else {
            false
        }
    }
}

pub fn run(catalog: Catalog, debug: bool, smoke: bool) -> Result<(), String> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let common = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&common);
        for class in [
            w!("AquaPetOverlay"),
            w!("AquaPetDebug"),
            w!("AquaPetSettings"),
        ] {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: module.into(),
                lpszClassName: class,
                hIcon: LoadIconW(Some(module.into()), w!("AQUA_ICON"))
                    .map_err(|e| e.to_string())?,
                hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err(windows::core::Error::from_thread().to_string());
            }
        }
        let pet = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("AquaPetOverlay"),
            w!("AQUA"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        let panel = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_COMPOSITED,
            w!("AquaPetDebug"),
            w!("AQUA · Debug 动作预览"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1180,
            900,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        SetWindowLongPtrW(panel, GWLP_USERDATA, 1);
        let settings = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_COMPOSITED,
            w!("AquaPetSettings"),
            w!("AQUA · 设置"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            SETTINGS_HEIGHT,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        SetWindowLongPtrW(settings, GWLP_USERDATA, 1);
        let monitor = MonitorFromWindow(panel, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let area = monitor_info.rcWork;
            let dpi = GetDpiForWindow(panel).max(96) as i32;
            let width = (1100 * dpi / 96).min(area.right - area.left - 32);
            let height = (860 * dpi / 96).min(area.bottom - area.top - 32);
            let _ = SetWindowPos(
                panel,
                None,
                area.left + (area.right - area.left - width) / 2,
                area.top + (area.bottom - area.top - height) / 2,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        APP.with(|slot| *slot.borrow_mut() = Some(App::new(catalog, pet, panel, settings, smoke)));
        let setup = APP.with(|slot| {
            let mut borrowed = slot.borrow_mut();
            let app = borrowed.as_mut().unwrap();
            app.restore_settings();
            app.create_settings_panel();
            app.update()?;
            app.tray = Some(tray::TrayIcon::new(pet)?);
            let _ = ShowWindow(pet, SW_SHOWNOACTIVATE);
            if debug || smoke {
                app.show_debug()?;
            }
            Ok::<(), windows::core::Error>(())
        });
        if let Err(error) = setup {
            return Err(error.to_string());
        }
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result == 0 {
                break;
            }
            if result < 0 {
                return Err(windows::core::Error::from_thread().to_string());
            }
            if matches!(message.message, WM_MOUSEMOVE | WM_MOUSELEAVE) {
                APP.with(|slot| {
                    let mut borrowed = slot.borrow_mut();
                    let app = borrowed.as_mut().unwrap();
                    if message.hwnd == app.search {
                        let mut point = message.pt;
                        let _ = ScreenToClient(panel, &mut point);
                        app.debug_pointer(app.search, message.message, point);
                    }
                });
            }
            if message.message == WM_KEYDOWN && message.wParam.0 == VK_TAB.0 as usize {
                let reverse = GetKeyState(VK_SHIFT.0 as i32) < 0;
                let handled = APP.with(|slot| {
                    let mut borrowed = slot.borrow_mut();
                    let app = borrowed.as_mut().unwrap();
                    if message.hwnd == settings {
                        app.settings_tab(reverse);
                        true
                    } else if message.hwnd == panel || message.hwnd == app.search {
                        app.debug_tab(reverse);
                        true
                    } else {
                        false
                    }
                });
                if handled {
                    continue;
                }
            }
            if message.message == WM_MOUSEWHEEL {
                let root = GetAncestor(message.hwnd, GA_ROOT);
                if (root == panel || root == settings) && root != message.hwnd {
                    let _ = SendMessageW(
                        root,
                        WM_MOUSEWHEEL,
                        Some(message.wParam),
                        Some(message.lParam),
                    );
                    continue;
                }
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        APP.with(|slot| {
            slot.borrow_mut().take();
        });
        Ok(())
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn old_settings_keep_position_and_default_walk_probability() {
        let settings: Settings =
            serde_json::from_str(r#"{"anchor_x":-400,"anchor_y":240,"scale":2}"#).unwrap();
        assert_eq!(
            (settings.anchor_x, settings.anchor_y, settings.scale),
            (-400, 240, 2)
        );
        assert_eq!(settings.walk_probability, DEFAULT_WALK_CHANCE);
        assert_eq!(settings.walk_distance_bias, DEFAULT_DISTANCE_BIAS);
        assert!(settings.dance_to_music);
    }

    #[test]
    fn saved_probability_restores_both_slider_endpoints() {
        for probability in [0, 25, 100] {
            let settings = Settings {
                anchor_x: 100,
                anchor_y: 200,
                scale: 4,
                walk_probability: probability,
                walk_distance_bias: DEFAULT_DISTANCE_BIAS,
                dance_to_music: default_dance_to_music(),
            };
            let bytes = serde_json::to_vec(&settings).unwrap();
            let restored: Settings = serde_json::from_slice(&bytes).unwrap();
            let mut wander = Wander::for_test(1);
            wander.set_probability(restored.walk_probability);
            assert_eq!(wander.probability(), probability);
        }
    }

    #[test]
    fn distance_preference_round_trips_and_upgrades_existing_settings() {
        let old: Settings = serde_json::from_str(
            r#"{"anchor_x":100,"anchor_y":200,"scale":4,"walk_probability":0}"#,
        )
        .unwrap();
        assert_eq!(old.walk_probability, 0);
        assert_eq!(old.walk_distance_bias, 50);
        for bias in [0, 50, 100] {
            let settings = Settings {
                walk_distance_bias: bias,
                ..old
            };
            let restored: Settings =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            let mut wander = Wander::for_test(1);
            wander.set_probability(restored.walk_probability);
            wander.set_distance_bias(restored.walk_distance_bias);
            assert_eq!(wander.distance_bias(), bias);
            assert_eq!(wander.probability(), 0);
        }
    }

    #[test]
    fn music_dance_preference_round_trips() {
        let old: Settings =
            serde_json::from_str(r#"{"anchor_x":100,"anchor_y":200,"scale":4}"#).unwrap();
        for dance_to_music in [false, true] {
            let settings = Settings {
                dance_to_music,
                ..old
            };
            let restored: Settings =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            assert_eq!(restored.dance_to_music, dance_to_music);
        }
    }
}
