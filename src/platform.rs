#![allow(unsafe_op_in_unsafe_fn)]
// Rust 1.98's lint also inspects the standard macro's internal lazy initializer.
#![allow(clippy::missing_const_for_thread_local)]

use crate::{
    catalog::Catalog,
    player::Player,
    render::{self, Bounds, Frame},
    wander::{DEFAULT_DISTANCE_BIAS, DEFAULT_WALK_CHANCE, Direction, Wander},
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
        System::{LibraryLoader::GetModuleHandleW, SystemServices::SS_RIGHT},
        UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};

const SEARCH: usize = 100;
const KIND: usize = 101;
const CATEGORY: usize = 102;
const LIST: usize = 103;
const COUNT: usize = 104;
const PLAY: usize = 200;
const RESTART: usize = 201;
const PREV: usize = 202;
const NEXT: usize = 203;
const LOOP: usize = 204;
const SPEED: usize = 205;
const RESET_SPEED: usize = 206;
const SCALE: usize = 207;
const BG: usize = 208;
const TIMELINE: usize = 209;
const STATUS: usize = 210;
const INFO: usize = 211;
const IDLE: usize = 212;
const TITLE: usize = 300;
const SEARCH_LABEL: usize = 301;
const SCALE_LABEL: usize = 302;
const SPEED_LABEL: usize = 303;
const BACKGROUND_LABEL: usize = 304;
const LOOP_LABEL: usize = 305;
const MENU_DEBUG: u32 = 400;
const MENU_EXIT: u32 = 401;
const MENU_SETTINGS: u32 = 402;
const WALK_CHANCE: usize = 500;
const WALK_LABEL: usize = 501;
const WALK_VALUE: usize = 502;
const WALK_OFF: usize = 503;
const WALK_OFTEN: usize = 504;
const SETTINGS_NOTE: usize = 505;
const DISTANCE_BIAS: usize = 506;
const DISTANCE_LABEL: usize = 507;
const DISTANCE_VALUE: usize = 508;
const DISTANCE_SHORT: usize = 509;
const DISTANCE_LONG: usize = 510;
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
unsafe fn combo_add(hwnd: HWND, s: &str) {
    let s = wide(s);
    send(hwnd, CB_ADDSTRING, 0, s.as_ptr() as isize);
}
unsafe fn combo_index(hwnd: HWND) -> usize {
    send(hwnd, CB_GETCURSEL, 0, 0).max(0) as usize
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
}
fn default_walk_chance() -> u8 {
    DEFAULT_WALK_CHANCE
}
fn default_distance_bias() -> u8 {
    DEFAULT_DISTANCE_BIAS
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

struct App {
    catalog: Catalog,
    player: Player,
    pet: HWND,
    debug: HWND,
    settings: HWND,
    controls: HashMap<usize, HWND>,
    font: HFONT,
    settings_font: HFONT,
    visible: Vec<usize>,
    categories: Vec<String>,
    bounds: Bounds,
    motion_bounds: Bounds,
    wander_bounds: Bounds,
    walk_clips: [usize; 4],
    wander: Wander,
    scale: u32,
    effective_scale: u32,
    anchor: POINT,
    drag: Option<(POINT, POINT)>,
    surface: Option<Surface>,
    last_clock: Instant,
    last_status: Instant,
    last_frame: Option<(usize, usize, bool, u32)>,
    last_position: Option<(i32, i32)>,
    preview: Option<Frame>,
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
        let walk_clips = Direction::ALL.map(|direction| {
            catalog
                .clips
                .iter()
                .position(|c| c.id == direction.clip_id())
                .expect("Missing verified walk clip")
        });
        // All automatic poses share one allowed area. Changing from a narrow
        // walk sprite to the wider idle must not push the pet sideways.
        let mut wander_bounds = bounds;
        for index in walk_clips {
            let b = render::clip_bounds(&catalog, &catalog.clips[index], true);
            wander_bounds.point(b.left, b.top);
            wander_bounds.point(b.right, b.bottom);
        }
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ std::process::id() as u64;
        let player = Player::new(idle, &catalog.clips[idle]);
        Self {
            catalog,
            player,
            pet,
            debug,
            settings,
            controls: HashMap::new(),
            font: HFONT::default(),
            settings_font: HFONT::default(),
            visible: vec![],
            categories: vec![],
            bounds,
            motion_bounds: wander_bounds,
            wander_bounds,
            walk_clips,
            wander: Wander::new(seed),
            scale: 4,
            effective_scale: 4,
            anchor: POINT { x: 0, y: 0 },
            drag: None,
            surface: None,
            last_clock: Instant::now(),
            last_status: Instant::now(),
            last_frame: None,
            last_position: None,
            preview: None,
            preview_rect: RECT::default(),
            preview_dirty: true,
            bg: 2,
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
    fn control(&self, id: usize) -> HWND {
        self.controls[&id]
    }
    unsafe fn create_controls(&mut self) -> windows::core::Result<()> {
        let module = GetModuleHandleW(None)?;
        let specs: [(usize, PCWSTR, &str, u32); 27] = [
            (TITLE, w!("STATIC"), "AQUA  /  动作预览", 0),
            (SEARCH_LABEL, w!("STATIC"), "搜索动作或原始资源名", 0),
            (
                SEARCH,
                w!("EDIT"),
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0,
            ),
            (
                KIND,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (
                CATEGORY,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (
                LIST,
                w!("LISTBOX"),
                "",
                LBS_NOTIFY as u32
                    | LBS_NOINTEGRALHEIGHT as u32
                    | WS_BORDER.0
                    | WS_VSCROLL.0
                    | WS_HSCROLL.0
                    | WS_TABSTOP.0,
            ),
            (COUNT, w!("STATIC"), "", 0),
            (
                PLAY,
                w!("BUTTON"),
                "暂停",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                RESTART,
                w!("BUTTON"),
                "重播",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                PREV,
                w!("BUTTON"),
                "上一帧",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                NEXT,
                w!("BUTTON"),
                "下一帧",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                IDLE,
                w!("BUTTON"),
                "默认晃动",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                LOOP,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (
                SPEED,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (
                RESET_SPEED,
                w!("BUTTON"),
                "恢复原速",
                BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            ),
            (
                SCALE,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (
                BG,
                w!("COMBOBOX"),
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            ),
            (TIMELINE, TRACKBAR_CLASSW, "", TBS_NOTICKS | WS_TABSTOP.0),
            (STATUS, w!("STATIC"), "", 0),
            (
                INFO,
                w!("EDIT"),
                "",
                ES_MULTILINE as u32
                    | ES_READONLY as u32
                    | ES_AUTOVSCROLL as u32
                    | WS_VSCROLL.0
                    | WS_BORDER.0
                    | WS_TABSTOP.0,
            ),
            (SCALE_LABEL, w!("STATIC"), "桌宠大小", 0),
            (SPEED_LABEL, w!("STATIC"), "播放速度", 0),
            (BACKGROUND_LABEL, w!("STATIC"), "预览背景", 0),
            (LOOP_LABEL, w!("STATIC"), "播放方式", 0),
            (306, w!("STATIC"), "选中后，桌宠与面板同步播放。", 0),
            (307, w!("STATIC"), "拖动桌宠调整位置 · 右键打开菜单", 0),
            (308, w!("STATIC"), "关闭此面板会回到默认晃动。", 0),
        ];
        for (id, class, label, style) in specs {
            let label = wide(label);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                PCWSTR(label.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                0,
                0,
                10,
                10,
                Some(self.debug),
                Some(HMENU(id as *mut c_void)),
                Some(module.into()),
                None,
            )?;
            self.controls.insert(id, hwnd);
        }
        for kind in ["全部", "原作动作", "原始素材", "未使用素材"] {
            combo_add(self.control(KIND), kind);
        }
        combo_add(self.control(CATEGORY), "全部分类");
        self.categories = self
            .catalog
            .clips
            .iter()
            .map(|c| c.category.clone())
            .collect();
        self.categories.sort();
        self.categories.dedup();
        for category in &self.categories {
            combo_add(self.control(CATEGORY), category);
        }
        for id in [KIND, CATEGORY] {
            send(self.control(id), CB_SETCURSEL, 0, 0);
        }
        for mode in ["单次", "循环"] {
            combo_add(self.control(LOOP), mode);
        }
        for rate in RATES {
            combo_add(self.control(SPEED), &format!("{rate}×"));
        }
        for scale in 1..=8 {
            combo_add(self.control(SCALE), &format!("{scale}× 原始像素"));
        }
        for bg in ["深色", "浅色", "棋盘格"] {
            combo_add(self.control(BG), bg);
        }
        send(self.control(BG), CB_SETCURSEL, self.bg, 0);
        send(self.control(TIMELINE), TBM_SETRANGEMIN, 0, 0);
        send(self.control(TIMELINE), TBM_SETRANGEMAX, 1, 10000);
        self.update_font();
        self.filter();
        self.sync_selection();
        self.layout();
        send(self.control(LIST), LB_SETTOPINDEX, 0, 0);
        self.create_settings_controls()?;
        Ok(())
    }
    unsafe fn create_settings_controls(&mut self) -> windows::core::Result<()> {
        let module = GetModuleHandleW(None)?;
        for (id, class, label, style) in [
            (WALK_LABEL, w!("STATIC"), "随机走动概率", 0),
            (WALK_VALUE, w!("STATIC"), "", SS_RIGHT.0),
            (
                WALK_CHANCE,
                TRACKBAR_CLASSW,
                "随机走动概率",
                TBS_NOTICKS | WS_TABSTOP.0,
            ),
            (WALK_OFF, w!("STATIC"), "不走动", 0),
            (WALK_OFTEN, w!("STATIC"), "经常走动", SS_RIGHT.0),
            (DISTANCE_LABEL, w!("STATIC"), "走动距离倾向", 0),
            (
                DISTANCE_BIAS,
                TRACKBAR_CLASSW,
                "走动距离倾向",
                TBS_NOTICKS | WS_TABSTOP.0,
            ),
            (DISTANCE_VALUE, w!("STATIC"), "", SS_RIGHT.0),
            (DISTANCE_SHORT, w!("STATIC"), "更倾向短路线", 0),
            (DISTANCE_LONG, w!("STATIC"), "更倾向长路线", SS_RIGHT.0),
            (
                SETTINGS_NOTE,
                w!("STATIC"),
                "每次距离仍随机，距离倾向从下次走动生效。\r\n设置自动保存，行走速度保持原速。\r\n停下时仍会左右摇晃。",
                0,
            ),
        ] {
            let label = wide(label);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                PCWSTR(label.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                0,
                0,
                10,
                10,
                Some(self.settings),
                Some(HMENU(id as *mut c_void)),
                Some(module.into()),
                None,
            )?;
            self.controls.insert(id, hwnd);
        }
        for id in [WALK_CHANCE, DISTANCE_BIAS] {
            send(self.control(id), TBM_SETRANGEMIN, 0, 0);
            send(self.control(id), TBM_SETRANGEMAX, 1, 100);
            send(self.control(id), TBM_SETPAGESIZE, 0, 10);
        }
        self.sync_walk_chance();
        self.sync_distance_bias();
        self.update_settings_font();
        self.layout_settings();
        Ok(())
    }
    unsafe fn sync_walk_chance(&self) {
        let probability = self.wander.probability();
        send(
            self.control(WALK_CHANCE),
            TBM_SETPOS,
            1,
            probability as isize,
        );
        text(self.control(WALK_VALUE), &format!("{probability}%"));
    }
    unsafe fn sync_distance_bias(&self) {
        let bias = self.wander.distance_bias();
        send(self.control(DISTANCE_BIAS), TBM_SETPOS, 1, bias as isize);
        self.update_distance_label();
    }
    unsafe fn update_distance_label(&self) {
        let label = match self.wander.distance_bias() {
            0..50 => "偏短",
            50 => "长短均衡",
            _ => "偏长",
        };
        text(self.control(DISTANCE_VALUE), label);
    }
    unsafe fn update_settings_font(&mut self) {
        let font = Self::make_font(self.settings);
        for (&id, &hwnd) in &self.controls {
            if id >= WALK_CHANCE {
                send(hwnd, WM_SETFONT, font.0 as usize, 1);
            }
        }
        if !self.settings_font.is_invalid() {
            let _ = DeleteObject(self.settings_font.into());
        }
        self.settings_font = font;
    }
    unsafe fn layout_settings(&self) {
        if !self.controls.contains_key(&WALK_CHANCE) {
            return;
        }
        let mut client = RECT::default();
        let _ = GetClientRect(self.settings, &mut client);
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        for (id, x, y, w, h) in [
            (WALK_LABEL, 24.0, 24.0, width - 140.0, 26.0),
            (WALK_VALUE, width - 104.0, 24.0, 80.0, 26.0),
            (WALK_CHANCE, 20.0, 64.0, width - 40.0, 32.0),
            (WALK_OFF, 24.0, 104.0, 100.0, 26.0),
            (WALK_OFTEN, width - 124.0, 104.0, 100.0, 26.0),
            (DISTANCE_LABEL, 24.0, 164.0, width - 160.0, 26.0),
            (DISTANCE_VALUE, width - 124.0, 164.0, 100.0, 26.0),
            (DISTANCE_BIAS, 20.0, 204.0, width - 40.0, 32.0),
            (DISTANCE_SHORT, 24.0, 244.0, 150.0, 26.0),
            (DISTANCE_LONG, width - 174.0, 244.0, 150.0, 26.0),
            (SETTINGS_NOTE, 24.0, 294.0, width - 48.0, 72.0),
        ] {
            let _ = MoveWindow(
                self.control(id),
                (x * dpi) as i32,
                (y * dpi) as i32,
                (w * dpi) as i32,
                (h * dpi) as i32,
                true,
            );
        }
    }
    unsafe fn show_settings(&mut self) {
        if !IsWindowVisible(self.settings).as_bool() {
            let work = self.work_area();
            let dpi = GetDpiForWindow(self.settings).max(96) as i32;
            let width = 480 * dpi / 96;
            let height = 420 * dpi / 96;
            let _ = SetWindowPos(
                self.settings,
                None,
                work.left + (work.right - work.left - width) / 2,
                work.top + (work.bottom - work.top - height) / 2,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        self.sync_walk_chance();
        self.sync_distance_bias();
        self.layout_settings();
        let _ = ShowWindow(self.settings, SW_RESTORE);
        let _ = SetForegroundWindow(self.settings);
        let _ = SetFocus(Some(self.control(WALK_CHANCE)));
    }
    unsafe fn update_font(&mut self) {
        let font = Self::make_font(self.debug);
        for (&id, &hwnd) in &self.controls {
            if id < WALK_CHANCE {
                send(hwnd, WM_SETFONT, font.0 as usize, 1);
            }
        }
        if !self.font.is_invalid() {
            let _ = DeleteObject(self.font.into());
        }
        self.font = font;
    }
    unsafe fn make_font(window: HWND) -> HFONT {
        let dpi = GetDpiForWindow(window).max(96);
        CreateFontW(
            -(((16 * dpi) / 96) as i32),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Microsoft YaHei UI"),
        )
    }
    unsafe fn layout(&mut self) {
        let mut client = RECT::default();
        let _ = GetClientRect(self.debug, &mut client);
        let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
        let w = client.right as f64 / dpi;
        let h = client.bottom as f64 / dpi;
        let left = 310.0_f64.min(w * 0.32);
        let right_x = left + 32.0;
        let rw = (w - right_x - 18.0).max(320.0);
        let place = |id, x: f64, y: f64, width: f64, height: f64| {
            let _ = MoveWindow(
                self.controls[&id],
                (x * dpi) as i32,
                (y * dpi) as i32,
                (width * dpi) as i32,
                (height * dpi) as i32,
                true,
            );
        };
        place(TITLE, 16.0, 14.0, w - 32.0, 28.0);
        place(SEARCH_LABEL, 16.0, 51.0, left, 23.0);
        place(SEARCH, 16.0, 77.0, left, 28.0);
        place(KIND, 16.0, 116.0, left, 260.0);
        place(CATEGORY, 16.0, 153.0, left, 400.0);
        place(LIST, 16.0, 193.0, left, (h - 264.0).max(150.0));
        place(COUNT, 16.0, h - 62.0, left, 24.0);
        place(308, 16.0, h - 34.0, left, 24.0);
        place(306, right_x, 52.0, rw, 24.0);
        let preview_bottom = (h - 365.0).max(150.0);
        self.preview_rect = RECT {
            left: (right_x * dpi) as i32,
            top: (82.0 * dpi) as i32,
            right: ((right_x + rw) * dpi) as i32,
            bottom: (preview_bottom * dpi) as i32,
        };
        let y = preview_bottom + 10.0;
        place(TIMELINE, right_x, y, rw, 28.0);
        place(STATUS, right_x, y + 32.0, rw, 24.0);
        let bw = ((rw - 32.0) / 5.0).min(110.0);
        for (i, id) in [PLAY, RESTART, PREV, NEXT, IDLE].iter().enumerate() {
            place(*id, right_x + i as f64 * (bw + 8.0), y + 62.0, bw, 31.0);
        }
        let cw = (rw - 24.0) / 4.0;
        for (i, (label, id)) in [
            (LOOP_LABEL, LOOP),
            (SPEED_LABEL, SPEED),
            (SCALE_LABEL, SCALE),
            (BACKGROUND_LABEL, BG),
        ]
        .iter()
        .enumerate()
        {
            let x = right_x + i as f64 * (cw + 8.0);
            place(*label, x, y + 105.0, cw, 23.0);
            place(*id, x, y + 130.0, cw, 260.0);
        }
        place(RESET_SPEED, right_x, y + 167.0, 110.0, 28.0);
        place(307, right_x + 120.0, y + 170.0, rw - 120.0, 23.0);
        place(INFO, right_x, y + 205.0, rw, (h - y - 220.0).max(70.0));
        self.preview_dirty = true;
        let _ = InvalidateRect(Some(self.debug), None, true);
    }
    unsafe fn filter(&mut self) {
        let search = window_text(self.control(SEARCH)).to_lowercase();
        let kind = combo_index(self.control(KIND));
        let category = combo_index(self.control(CATEGORY));
        self.visible = self
            .catalog
            .clips
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let kind_ok = match kind {
                    1 => c.kind == "action",
                    2 => c.kind == "resource",
                    3 => c.category.contains("Unused"),
                    _ => true,
                };
                kind_ok
                    && (category == 0 || self.categories.get(category - 1) == Some(&c.category))
                    && (search.is_empty()
                        || format!("{} {} {} {}", c.id, c.label, c.category, c.notes)
                            .to_lowercase()
                            .contains(&search)
                        || c.steps
                            .iter()
                            .flat_map(|s| &s.layers)
                            .any(|l| l.sprite.to_lowercase().contains(&search)))
            })
            .map(|(i, _)| i)
            .collect();
        let list = self.control(LIST);
        send(list, WM_SETREDRAW, 0, 0);
        send(list, LB_RESETCONTENT, 0, 0);
        for &index in &self.visible {
            let c = &self.catalog.clips[index];
            let label = wide(&format!(
                "{} {}",
                if c.kind == "resource" {
                    "[素材]"
                } else if c.verified {
                    "[原作]"
                } else {
                    "[待核实]"
                },
                c.label
            ));
            send(list, LB_ADDSTRING, 0, label.as_ptr() as isize);
        }
        send(list, LB_SETHORIZONTALEXTENT, 1000, 0);
        if let Some(i) = self.visible.iter().position(|i| *i == self.player.selected) {
            send(list, LB_SETCURSEL, i, 0);
        }
        send(list, WM_SETREDRAW, 1, 0);
        let _ = InvalidateRect(Some(list), None, true);
        text(
            self.control(COUNT),
            &format!("{} 项 · 94 组素材 / 346 张原图", self.visible.len()),
        );
    }
    unsafe fn work_area(&self) -> RECT {
        let monitor = MonitorFromPoint(self.anchor, MONITOR_DEFAULTTONEAREST);
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
    unsafe fn work_bounds(&self) -> Bounds {
        let area = self.work_area();
        Bounds {
            left: area.left as f64,
            top: area.top as f64,
            right: area.right as f64,
            bottom: area.bottom as f64,
        }
    }
    unsafe fn fit(&mut self) {
        let ((x, y), scale) = crate::placement::fit(
            (self.anchor.x, self.anchor.y),
            self.scale,
            self.motion_bounds,
            self.work_bounds(),
        );
        self.anchor = POINT { x, y };
        self.effective_scale = scale;
    }
    unsafe fn restore_settings(&mut self) {
        let settings = std::fs::read(settings_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok());
        if let Some(s) = settings {
            self.scale = s.scale.clamp(1, 8);
            self.wander.set_probability(s.walk_probability);
            self.wander.set_distance_bias(s.walk_distance_bias);
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
        self.fit();
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
        };
        if let (Some(parent), Ok(bytes)) = (path.parent(), serde_json::to_vec_pretty(&data))
            && std::fs::create_dir_all(parent).is_ok()
        {
            let _ = std::fs::write(path, bytes);
        }
    }
    unsafe fn sync_selection(&mut self) {
        if self.controls.is_empty() {
            return;
        }
        let c = &self.catalog.clips[self.player.selected];
        let resources = c
            .steps
            .iter()
            .flat_map(|s| &s.layers)
            .map(|l| l.sprite.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\r\n");
        let details = format!(
            "{}\r\n{}\r\n\r\n原始资源：\r\n{}\r\n\r\n{}\r\n\r\n原作依据：\r\n{}\r\n游戏：30 ticks/s · data.win SHA-256 {}",
            c.label,
            if c.kind == "resource" {
                "原始资源预览（不是原作动作的播放速度）"
            } else if c.verified {
                "片段已对照原作脚本（尚未逐帧录像比对）"
            } else {
                "待核实：请查看说明"
            },
            resources,
            c.notes,
            c.evidence.join("\r\n"),
            self.catalog.source_hash
        );
        text(self.control(INFO), &details);
        send(
            self.control(LOOP),
            CB_SETCURSEL,
            usize::from(self.player.looping),
            0,
        );
        send(self.control(SPEED), CB_SETCURSEL, 3, 0);
        send(
            self.control(SCALE),
            CB_SETCURSEL,
            (self.scale - 1) as usize,
            0,
        );
        if let Some(i) = self.visible.iter().position(|i| *i == self.player.selected) {
            send(self.control(LIST), LB_SETCURSEL, i, 0);
        }
        self.last_status = Instant::now() - Duration::from_secs(1);
    }
    unsafe fn select(&mut self, index: usize) {
        self.player.select(index, &self.catalog.clips[index]);
        self.bounds = render::clip_bounds(&self.catalog, &self.catalog.clips[index], false);
        self.motion_bounds = if self.debug_open {
            render::clip_bounds(&self.catalog, &self.catalog.clips[index], true)
        } else {
            self.wander_bounds
        };
        self.last_frame = None;
        self.preview_dirty = true;
        self.fit();
        if self.debug_open {
            self.sync_selection();
        }
        self.last_clock = Instant::now();
    }
    unsafe fn reset_wander(&mut self) {
        self.wander.reset();
        self.select(self.catalog.idle_index());
    }
    unsafe fn show_debug(&mut self) {
        let was_open = self.debug_open;
        self.debug_open = true;
        if !was_open {
            self.reset_wander();
        }
        self.preview_dirty = true;
        let _ = ShowWindow(self.debug, SW_RESTORE);
        let _ = SetForegroundWindow(self.debug);
        self.layout();
    }
    unsafe fn close_debug(&mut self) {
        self.debug_open = false;
        let _ = ShowWindow(self.debug, SW_HIDE);
        self.reset_wander();
        self.save_settings();
    }
    unsafe fn exit(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.save_settings();
        let _ = KillTimer(Some(self.pet), 1);
        let _ = DestroyWindow(self.debug);
        let _ = DestroyWindow(self.settings);
        let _ = DestroyWindow(self.pet);
        PostQuitMessage(0);
    }
    unsafe fn menu(&mut self) {
        if !self.debug_open {
            self.reset_wander();
        }
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS as usize, w!("设置"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_DEBUG as usize, w!("Debug · 动作预览"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, w!("关闭"));
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
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
        let _ = DestroyMenu(menu);
        self.last_clock = Instant::now();
        if command == MENU_SETTINGS {
            self.show_settings();
        } else if command == MENU_DEBUG {
            self.show_debug();
        } else if command == MENU_EXIT {
            self.exit();
        }
    }
    unsafe fn command(&mut self, wp: WPARAM) {
        let id = wp.0 & 0xffff;
        let notification = (wp.0 >> 16) & 0xffff;
        if (id == SEARCH && notification == EN_CHANGE as usize)
            || ([KIND, CATEGORY].contains(&id) && notification == CBN_SELCHANGE as usize)
        {
            self.filter();
            return;
        }
        if id == LIST && notification == LBN_SELCHANGE as usize {
            let i = send(self.control(LIST), LB_GETCURSEL, 0, 0);
            if i >= 0
                && let Some(&index) = self.visible.get(i as usize)
            {
                self.select(index);
            }
            return;
        }
        let clip = &self.catalog.clips[self.player.selected];
        match id {
            PLAY => self.player.toggle(),
            RESTART => self.player.restart(),
            PREV => self.player.step(clip, false),
            NEXT => self.player.step(clip, true),
            LOOP if notification == CBN_SELCHANGE as usize => {
                self.player.looping = combo_index(self.control(LOOP)) == 1
            }
            SPEED if notification == CBN_SELCHANGE as usize => {
                self.player.rate = RATES[combo_index(self.control(SPEED)).min(6)]
            }
            RESET_SPEED => {
                self.player.rate = 1.0;
                send(self.control(SPEED), CB_SETCURSEL, 3, 0);
            }
            SCALE if notification == CBN_SELCHANGE as usize => {
                self.scale = combo_index(self.control(SCALE)) as u32 + 1;
                self.fit();
                self.last_frame = None;
                self.save_settings();
            }
            BG if notification == CBN_SELCHANGE as usize => self.bg = combo_index(self.control(BG)),
            IDLE => {
                self.select(self.catalog.idle_index());
            }
            _ => return,
        }
        self.preview_dirty = true;
        self.last_status = Instant::now() - Duration::from_secs(1);
        self.last_clock = Instant::now();
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
        // Resuming from sleep or a suspended process should not jump across an entire scene.
        if dt < 2.0 {
            if !self.debug_open && self.drag.is_none() {
                let limits = crate::placement::anchor_limits(
                    self.effective_scale,
                    self.wander_bounds,
                    self.work_bounds(),
                );
                let (x, y) = self.wander.advance(
                    dt,
                    self.catalog.game_fps,
                    self.effective_scale,
                    (self.anchor.x, self.anchor.y),
                    limits,
                );
                self.anchor = POINT { x, y };
                let selected = self
                    .wander
                    .direction()
                    .map(|d| self.walk_clips[d as usize])
                    .unwrap_or_else(|| self.catalog.idle_index());
                if self.player.selected != selected {
                    self.select(selected);
                    // select() resets the preview clock; automatic walking must
                    // still account for all time between timer callbacks.
                    self.last_clock = now;
                }
                let clip = &self.catalog.clips[selected];
                self.player.tick =
                    (self.wander.animation_seconds() * self.catalog.game_fps) % clip.duration();
            } else {
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
            let frame = render::render(
                &self.catalog,
                &clip.steps[index],
                self.bounds,
                self.effective_scale as f64,
                false,
            );
            if self
                .surface
                .as_ref()
                .is_none_or(|s| s.width != frame.width as i32 || s.height != frame.height as i32)
            {
                self.surface = Some(Surface::new(frame.width as i32, frame.height as i32)?);
            }
            let pixels = frame.bgra_premultiplied();
            let surface = self.surface.as_ref().unwrap();
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), surface.bits.cast::<u8>(), pixels.len());
            self.last_frame = Some(key);
            self.preview_dirty = true;
        }
        if let Some(surface) = &self.surface {
            let (rx, ry) = self.player.root(clip);
            let s = self.effective_scale as f64;
            let dst = POINT {
                x: self.anchor.x + ((self.bounds.left + rx) * s).round() as i32,
                y: self.anchor.y + ((self.bounds.top + ry) * s).round() as i32,
            };
            let size = SIZE {
                cx: surface.width,
                cy: surface.height,
            };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            if changed || self.last_position != Some((dst.x, dst.y)) {
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
                self.last_position = Some((dst.x, dst.y));
            }
        }
        if self.debug_open && self.preview_dirty {
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
            self.preview = Some(render::render_preview(
                &self.catalog,
                &step,
                self.motion_bounds,
                scale,
                self.bg,
            ));
            self.preview_dirty = false;
            let _ = InvalidateRect(Some(self.debug), Some(&self.preview_rect), false);
        }
        if self.debug_open && now.duration_since(self.last_status) >= Duration::from_millis(100) {
            let clip = &self.catalog.clips[self.player.selected];
            let frame_number = clip.steps[index]
                .layers
                .last()
                .map_or("空白".into(), |l| (l.frame + 1).to_string());
            text(
                self.control(PLAY),
                if self.player.waiting.is_some() {
                    "继续"
                } else if self.player.playing {
                    "暂停"
                } else {
                    "播放"
                },
            );
            let state = if self.player.waiting.is_some() {
                "等待手动继续"
            } else if self.player.finished {
                "完成 · 已回起点"
            } else if self.player.playing {
                "播放中"
            } else {
                "暂停"
            };
            text(
                self.control(STATUS),
                &format!(
                    "{} · {:.2}/{:.2}s · 样本 {}/{} · 图帧 {} · {:.2}× · 桌宠 {}×{}",
                    state,
                    self.player.tick / 30.0,
                    clip.duration() / 30.0,
                    index + 1,
                    clip.steps.len(),
                    frame_number,
                    self.player.rate,
                    self.effective_scale,
                    if self.effective_scale < self.scale {
                        "（适应屏幕）"
                    } else {
                        ""
                    }
                ),
            );
            send(
                self.control(TIMELINE),
                TBM_SETPOS,
                1,
                (self.player.tick / clip.duration() * 10000.0).round() as isize,
            );
            self.last_status = now;
        }
        Ok(())
    }
    unsafe fn paint(&self) {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(self.debug, &mut ps);
        let color = match self.bg {
            0 => COLORREF(0x2c221b),
            1 => COLORREF(0xeaf0f0),
            _ => COLORREF(0xded8d2),
        };
        let brush = CreateSolidBrush(color);
        FillRect(dc, &self.preview_rect, brush);
        let _ = DeleteObject(brush.into());
        if let Some(frame) = &self.preview {
            let x = self.preview_rect.left
                + (self.preview_rect.right - self.preview_rect.left - frame.width as i32) / 2;
            let y = self.preview_rect.top
                + (self.preview_rect.bottom - self.preview_rect.top - frame.height as i32) / 2;
            let pixels = frame.on_background(self.bg);
            let info = bitmap_info(frame.width as i32, frame.height as i32);
            SetDIBitsToDevice(
                dc,
                x,
                y,
                frame.width as u32,
                frame.height as u32,
                0,
                0,
                0,
                frame.height as u32,
                pixels.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
            );
        }
        let _ = EndPaint(self.debug, &ps);
    }
    unsafe fn handle(&mut self, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let debug = hwnd == self.debug;
        let pet = hwnd == self.pet;
        let settings = hwnd == self.settings;
        match msg {
            SHOW_DEBUG if pet => {
                self.show_debug();
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
                self.menu();
                Some(LRESULT(0))
            }
            WM_MOUSEACTIVATE if pet => Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_LBUTTONDOWN if pet => {
                if !self.debug_open {
                    self.reset_wander();
                }
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                self.drag = Some((p, self.anchor));
                SetCapture(self.pet);
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE if pet => {
                if let Some((start, anchor)) = self.drag {
                    let mut p = POINT::default();
                    let _ = GetCursorPos(&mut p);
                    self.anchor = POINT {
                        x: anchor.x + p.x - start.x,
                        y: anchor.y + p.y - start.y,
                    };
                    let _ = self.update();
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP if pet => {
                self.drag = None;
                let _ = ReleaseCapture();
                if !self.debug_open {
                    self.reset_wander();
                }
                self.fit();
                self.last_frame = None;
                self.save_settings();
                Some(LRESULT(0))
            }
            WM_CAPTURECHANGED if pet => {
                if self.drag.take().is_some() {
                    if !self.debug_open {
                        self.reset_wander();
                    }
                    self.fit();
                    self.last_frame = None;
                    self.save_settings();
                }
                Some(LRESULT(0))
            }
            WM_COMMAND if debug => {
                self.command(wp);
                Some(LRESULT(0))
            }
            WM_HSCROLL if settings && lp.0 == self.control(WALK_CHANCE).0 as isize => {
                let probability =
                    send(self.control(WALK_CHANCE), WM_USER, 0, 0).clamp(0, 100) as u8;
                self.wander.set_probability(probability);
                text(self.control(WALK_VALUE), &format!("{probability}%"));
                if probability == 0 && !self.debug_open {
                    self.select(self.catalog.idle_index());
                }
                // Apply while dragging; save on release, keyboard or page clicks.
                if wp.0 & 0xffff != TB_THUMBTRACK as usize {
                    self.save_settings();
                }
                Some(LRESULT(0))
            }
            WM_HSCROLL if debug && lp.0 == self.control(TIMELINE).0 as isize => {
                // TBM_GETPOS is WM_USER; windows 0.62 does not export its named constant.
                let position = send(self.control(TIMELINE), WM_USER, 0, 0);
                let clip = &self.catalog.clips[self.player.selected];
                self.player
                    .seek(clip, clip.duration() * position as f64 / 10000.0);
                self.preview_dirty = true;
                Some(LRESULT(0))
            }
            WM_HSCROLL if settings && lp.0 == self.control(DISTANCE_BIAS).0 as isize => {
                let bias = send(self.control(DISTANCE_BIAS), WM_USER, 0, 0).clamp(0, 100) as u8;
                self.wander.set_distance_bias(bias);
                self.update_distance_label();
                if wp.0 & 0xffff != TB_THUMBTRACK as usize {
                    self.save_settings();
                }
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
            WM_PAINT if debug => {
                self.paint();
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
                self.fit();
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
                    self.fit();
                    self.last_frame = None;
                }
                Some(LRESULT(0))
            }
            WM_GETMINMAXINFO if debug => {
                let info = &mut *(lp.0 as *mut MINMAXINFO);
                let dpi = GetDpiForWindow(hwnd).max(96) as i32;
                info.ptMinTrackSize = POINT {
                    x: 800 * dpi / 96,
                    y: 550 * dpi / 96,
                };
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        for font in [self.font, self.settings_font] {
            if !font.is_invalid() {
                unsafe {
                    let _ = DeleteObject(font.into());
                }
            }
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
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
        for (class, background) in [
            (w!("AquaPetOverlay"), HBRUSH::default()),
            (
                w!("AquaPetDebug"),
                HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut c_void),
            ),
            (
                w!("AquaPetSettings"),
                HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut c_void),
            ),
        ] {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: module.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?,
                hbrBackground: background,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err(windows::core::Error::from_thread().to_string());
            }
        }
        let pet = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
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
            WS_EX_APPWINDOW,
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
        let settings = CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("AquaPetSettings"),
            w!("AQUA · 设置"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            420,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        let monitor = MonitorFromWindow(panel, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let area = monitor_info.rcWork;
            let dpi = GetDpiForWindow(panel).max(96) as i32;
            let width = (1100 * dpi / 96).min(area.right - area.left - 32);
            let height = (800 * dpi / 96).min(area.bottom - area.top - 32);
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
            app.create_controls()?;
            app.update()?;
            let _ = ShowWindow(pet, SW_SHOWNOACTIVATE);
            if debug || smoke {
                app.show_debug();
            }
            SetTimer(Some(pet), 1, 16, None);
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
            if !(IsWindowVisible(panel).as_bool() && IsDialogMessageW(panel, &message).as_bool()
                || IsWindowVisible(settings).as_bool()
                    && IsDialogMessageW(settings, &message).as_bool())
            {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
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
            };
            let bytes = serde_json::to_vec(&settings).unwrap();
            let restored: Settings = serde_json::from_slice(&bytes).unwrap();
            let mut wander = Wander::new(1);
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
            let mut wander = Wander::new(1);
            wander.set_probability(restored.walk_probability);
            wander.set_distance_bias(restored.walk_distance_bias);
            assert_eq!(wander.distance_bias(), bias);
            assert_eq!(wander.probability(), 0);
        }
    }
}
