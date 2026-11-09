use super::*;

const ORDER: [usize; 5] = [WALK_CHANCE, DISTANCE_BIAS, DANCE_TO_MUSIC, SAVE, BACK];
const CLOSE: usize = 509;
pub(super) const SAVE: usize = 510;
const PANEL_MARGIN: f64 = 16.0;
const PANEL_TOP: f64 = 40.0;
const TITLE_TOP: f64 = 16.0;
const FIRST_ROW_TOP: f64 = 69.0;
const ROW_HEIGHT: f64 = 34.0;
const STEP: i32 = 5;

#[derive(PartialEq, Eq)]
pub(super) enum SaveState {
    Editing,
    Saved,
    Failed,
}

pub(super) struct Row {
    pub(super) bounds: RECT,
}

fn panel_rect(dpi: f64, width: f64, height: f64) -> RECT {
    ui::rect(
        dpi,
        PANEL_MARGIN,
        PANEL_TOP,
        (width - PANEL_MARGIN * 2.0).max(1.0),
        (height - PANEL_TOP - PANEL_MARGIN).max(1.0),
    )
}

impl App {
    pub unsafe fn create_settings_panel(&mut self) {
        self.settings_draft = self.preferences();
        self.update_settings_font();
        self.layout_settings();
    }

    pub unsafe fn update_settings_font(&mut self) {
        self.settings_fonts = ui::PixelFonts::new(self.settings);
    }

    pub unsafe fn layout_settings(&mut self) {
        let mut client = RECT::default();
        let _ = GetClientRect(self.settings, &mut client);
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        self.settings_regions.clear();
        for (index, id) in ORDER.into_iter().enumerate() {
            let y = PANEL_TOP + FIRST_ROW_TOP + index as f64 * ROW_HEIGHT;
            self.settings_regions.insert(
                id,
                Row {
                    bounds: ui::rect(
                        dpi,
                        PANEL_MARGIN + 16.0,
                        y,
                        width - (PANEL_MARGIN + 16.0) * 2.0,
                        ROW_HEIGHT,
                    ),
                },
            );
        }
        self.settings_regions.insert(
            CLOSE,
            Row {
                bounds: ui::rect(dpi, width - PANEL_MARGIN - 24.0, 4.0, 24.0, 24.0),
            },
        );
        let _ = InvalidateRect(Some(self.settings), None, false);
    }

    pub unsafe fn show_settings(&mut self) {
        if !IsWindowVisible(self.settings).as_bool() {
            self.settings_ui.focus = Some(WALK_CHANCE);
            self.settings_ui.hot = None;
            self.settings_ui.active = None;
            let work = self.work_area();
            let dpi = GetDpiForWindow(self.settings).max(96) as i32;
            let width = SETTINGS_WIDTH * dpi / 96;
            let height = SETTINGS_HEIGHT * dpi / 96;
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
        let _ = ShowWindow(self.settings, SW_RESTORE);
        let _ = SetForegroundWindow(self.settings);
        let _ = SetFocus(Some(self.settings));
        self.layout_settings();
    }

    pub unsafe fn paint_settings(&self, dc: HDC, client: RECT) {
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let height = client.bottom as f64 / dpi;
        if self.settings_save == SaveState::Saved {
            self.paint_save_result(dc, dpi, width, height);
        } else {
            self.paint_setting_rows(dc, dpi, width, height);
        }
        ui::close(
            dc,
            dpi,
            self.settings_regions[&CLOSE].bounds,
            if self.settings_ui.hot_control() == Some(CLOSE) {
                ui::YELLOW
            } else {
                ui::WHITE
            },
        );
    }

    unsafe fn paint_setting_rows(&self, dc: HDC, dpi: f64, width: f64, height: f64) {
        let panel = panel_rect(dpi, width, height);
        ui::border(dc, panel, ui::WHITE, (4.0 * dpi).round() as i32);
        for (x, y) in [
            (panel.left as f64 / dpi, panel.top as f64 / dpi),
            (panel.right as f64 / dpi, panel.top as f64 / dpi),
            (panel.left as f64 / dpi, panel.bottom as f64 / dpi),
            (panel.right as f64 / dpi, panel.bottom as f64 / dpi),
        ] {
            ui::star(dc, dpi, x, y);
        }
        ui::centered_label(
            dc,
            ui::rect(
                dpi,
                panel.left as f64 / dpi,
                panel.top as f64 / dpi + TITLE_TOP,
                (panel.right - panel.left) as f64 / dpi,
                32.0,
            ),
            "设置",
            self.settings_fonts.font,
            ui::WHITE,
        );
        for id in ORDER {
            let bounds = self.settings_regions[&id].bounds;
            let color = if self.settings_ui.selected(id) {
                ui::YELLOW
            } else {
                ui::WHITE
            };
            if self.settings_ui.selected(id) {
                ui::heart(
                    dc,
                    dpi,
                    bounds.left as f64 / dpi + 24.0,
                    bounds.top as f64 / dpi + 11.0,
                );
            }
            let name = match id {
                WALK_CHANCE => "主动走跳",
                DISTANCE_BIAS => "散步距离",
                DANCE_TO_MUSIC => "随音乐舞动",
                SAVE => "保存",
                _ => "返回",
            };
            ui::label(
                dc,
                RECT {
                    left: bounds.left + (48.0 * dpi) as i32,
                    right: bounds.right - (150.0 * dpi) as i32,
                    ..bounds
                },
                name,
                self.settings_fonts.font,
                color,
            );
            if let Some(value) = self.setting_text(id) {
                let mut value_rect = RECT {
                    left: bounds.right - (150.0 * dpi) as i32,
                    right: bounds.right - (40.0 * dpi) as i32,
                    ..bounds
                };
                ui::text(
                    dc,
                    &mut value_rect,
                    &value,
                    self.settings_fonts.font,
                    color,
                    DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_NOPREFIX,
                );
            }
        }
    }

    unsafe fn paint_save_result(&self, dc: HDC, dpi: f64, width: f64, height: f64) {
        static FRAME: std::sync::OnceLock<(BITMAPINFO, Vec<u8>)> = std::sync::OnceLock::new();
        let (bitmap, pixels) = FRAME.get_or_init(|| {
            let mut frame =
                image::load_from_memory(include_bytes!("../../assets/ui/save-frame.png"))
                    .expect("embedded save frame")
                    .into_rgba8();
            let bitmap = bitmap_info(frame.width() as i32, frame.height() as i32);
            for pixel in frame.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            (bitmap, frame.into_raw())
        });
        let frame_width = f64::from(bitmap.bmiHeader.biWidth * 2);
        let frame_height = f64::from(-bitmap.bmiHeader.biHeight * 2);
        let x = (width - frame_width) / 2.0;
        let y = (PANEL_TOP + height - PANEL_MARGIN - frame_height) / 2.0;
        let rect = ui::rect(dpi, x, y, frame_width, frame_height);
        SetStretchBltMode(dc, COLORONCOLOR);
        StretchDIBits(
            dc,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            0,
            0,
            bitmap.bmiHeader.biWidth,
            -bitmap.bmiHeader.biHeight,
            Some(pixels.as_ptr().cast()),
            bitmap,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        let slot_top = 96.0;
        for (label, top, height) in [
            ("设置", 0.0, 88.0),
            ("已保存存档", slot_top, frame_height - slot_top),
        ] {
            ui::centered_label(
                dc,
                ui::rect(dpi, x, y + top, frame_width, height),
                label,
                self.settings_fonts.font,
                ui::YELLOW,
            );
        }
    }

    fn setting_amount(&self, id: usize) -> Option<i32> {
        match id {
            WALK_CHANCE => Some(self.settings_draft.walk_probability as i32),
            DISTANCE_BIAS => Some(self.settings_draft.walk_distance_bias as i32),
            _ => None,
        }
    }

    fn setting_text(&self, id: usize) -> Option<String> {
        Some(match id {
            WALK_CHANCE => format!("{}%", self.settings_draft.walk_probability),
            DISTANCE_BIAS => format!("{}%", self.settings_draft.walk_distance_bias),
            DANCE_TO_MUSIC if self.settings_draft.dance_to_music => "开".into(),
            DANCE_TO_MUSIC => "关".into(),
            SAVE if self.settings_save == SaveState::Failed => "保存失败".into(),
            _ => return None,
        })
    }

    unsafe fn toggle_music_dance(&mut self) {
        self.settings_draft.dance_to_music = !self.settings_draft.dance_to_music;
        let _ = InvalidateRect(
            Some(self.settings),
            Some(&self.settings_regions[&DANCE_TO_MUSIC].bounds),
            false,
        );
    }

    unsafe fn activate_setting(&mut self, id: usize) {
        match id {
            DANCE_TO_MUSIC => {
                sound::menu_confirm();
                self.toggle_music_dance();
            }
            SAVE => {
                sound::menu_confirm();
                let draft = self.settings_draft;
                self.settings_save = if self.smoke || self.write_settings(draft).is_ok() {
                    self.apply_preferences(draft);
                    self.settings_ui.hot = None;
                    SaveState::Saved
                } else {
                    SaveState::Failed
                };
                let _ = InvalidateRect(Some(self.settings), None, false);
            }
            BACK => {
                sound::menu_confirm();
                self.close_settings();
            }
            CLOSE => self.close_settings(),
            _ => {}
        }
    }

    fn setting_at(&self, point: POINT) -> Option<usize> {
        if self.settings_save == SaveState::Saved {
            return ui::contains(&self.settings_regions[&CLOSE].bounds, point).then_some(CLOSE);
        }
        self.settings_regions
            .iter()
            .find_map(|(&id, row)| ui::contains(&row.bounds, point).then_some(id))
    }

    unsafe fn set_settings_hover(&mut self, next: Option<usize>) {
        let old_hot = self.settings_ui.hot_control();
        if old_hot == next {
            return;
        }
        let old_focus = self.settings_ui.focus;
        self.settings_ui.hot = next.map(ui::Hover::Control);
        if next.is_some_and(|id| id != CLOSE) {
            self.settings_ui.focus = next;
            if next != old_focus {
                sound::menu_move();
            }
        }
        for id in [old_hot, old_focus, next, self.settings_ui.focus]
            .into_iter()
            .flatten()
        {
            let _ = InvalidateRect(
                Some(self.settings),
                Some(&self.settings_regions[&id].bounds),
                false,
            );
        }
    }

    fn set_setting(&mut self, id: usize, value: i32) {
        let value = value.clamp(0, 100) as u8;
        let current = match id {
            WALK_CHANCE => &mut self.settings_draft.walk_probability,
            DISTANCE_BIAS => &mut self.settings_draft.walk_distance_bias,
            _ => return,
        };
        if *current == value {
            return;
        }
        *current = value;
        sound::menu_move();
        unsafe {
            let _ = InvalidateRect(
                Some(self.settings),
                Some(&self.settings_regions[&id].bounds),
                false,
            );
        }
    }

    unsafe fn adjust_setting(&mut self, id: usize, delta: i32) {
        if let Some(current) = self.setting_amount(id) {
            self.set_setting(id, current + delta);
        }
    }

    unsafe fn move_focus(&mut self, delta: i32) {
        if self.settings_save == SaveState::Saved {
            return;
        }
        let current = ORDER
            .iter()
            .position(|id| Some(*id) == self.settings_ui.focus)
            .unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(ORDER.len() as i32);
        self.settings_ui.focus = Some(ORDER[next as usize]);
        self.settings_ui.hot = None;
        sound::menu_move();
        let _ = SetFocus(Some(self.settings));
        let _ = InvalidateRect(Some(self.settings), None, false);
    }

    pub(super) unsafe fn close_settings(&mut self) {
        self.settings_draft = self.preferences();
        self.settings_save = SaveState::Editing;
        if self.settings_ui.active.take().is_some() {
            let _ = ReleaseCapture();
        }
        let _ = ShowWindow(self.settings, SW_HIDE);
    }

    unsafe fn dismiss_save_result(&mut self) {
        self.settings_save = SaveState::Editing;
        let _ = InvalidateRect(Some(self.settings), None, false);
    }

    pub unsafe fn settings_hit_test(&self, lp: LPARAM) -> LRESULT {
        let mut point = POINT {
            x: lp.0 as u16 as i16 as i32,
            y: (lp.0 >> 16) as u16 as i16 as i32,
        };
        let _ = ScreenToClient(self.settings, &mut point);
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let close = self
            .settings_regions
            .get(&CLOSE)
            .is_some_and(|row| ui::contains(&row.bounds, point));
        if !close && point.y < (PANEL_TOP * dpi).round() as i32 {
            LRESULT(HTCAPTION as isize)
        } else {
            LRESULT(HTCLIENT as isize)
        }
    }

    pub unsafe fn settings_mouse(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) {
        let point = ui::point(lp);
        match msg {
            WM_MOUSEMOVE => {
                self.settings_ui.track_pointer(self.settings);
                self.set_settings_hover(self.setting_at(point));
            }
            WM_MOUSELEAVE if self.settings_ui.pointer_left(self.settings) => {
                self.set_settings_hover(None)
            }
            WM_LBUTTONDOWN => {
                if let Some(id) = self.setting_at(point) {
                    if id != CLOSE {
                        self.settings_ui.focus = Some(id);
                    }
                    self.settings_ui.hot = Some(ui::Hover::Control(id));
                    let _ = SetFocus(Some(self.settings));
                    self.settings_ui.active = Some(id);
                    let _ = SetCapture(self.settings);
                    if let Some(current) = self.setting_amount(id) {
                        let bounds = self.settings_regions[&id].bounds;
                        let delta = if point.x < (bounds.left + bounds.right) / 2 {
                            -STEP
                        } else {
                            STEP
                        };
                        self.set_setting(id, current + delta);
                    }
                    let _ = InvalidateRect(Some(self.settings), None, false);
                }
            }
            WM_LBUTTONUP => {
                if let Some(id) = self.settings_ui.active.take() {
                    let _ = ReleaseCapture();
                    if self.setting_at(point) == Some(id) {
                        self.activate_setting(id);
                    }
                } else if self.settings_save == SaveState::Saved {
                    self.dismiss_save_result();
                }
            }
            WM_MOUSEWHEEL if self.settings_save != SaveState::Saved => {
                let delta = (wp.0 >> 16) as u16 as i16 as i32 / 120;
                if let Some(id) = self.settings_ui.hot_control().or(self.settings_ui.focus) {
                    self.adjust_setting(id, delta * STEP);
                }
            }
            _ => {}
        }
    }

    pub unsafe fn settings_key(&mut self, key: VIRTUAL_KEY) {
        if self.settings_save == SaveState::Saved {
            if matches!(key, VK_RETURN | VK_SPACE | VK_ESCAPE) {
                self.dismiss_save_result();
            }
            return;
        }
        if key == VK_ESCAPE {
            self.close_settings();
            return;
        }
        let Some(id) = self.settings_ui.focus else {
            return;
        };
        if key == VK_UP || key == VK_DOWN {
            self.move_focus(if key == VK_UP { -1 } else { 1 });
            return;
        }
        if key == VK_RETURN || key == VK_SPACE {
            self.activate_setting(id);
            return;
        }
        if id == DANCE_TO_MUSIC {
            if key == VK_LEFT || key == VK_RIGHT {
                let on = key == VK_RIGHT;
                if self.settings_draft.dance_to_music != on {
                    sound::menu_move();
                    self.toggle_music_dance();
                }
            }
            return;
        }
        let Some(current) = self.setting_amount(id) else {
            return;
        };
        let value = match key {
            key if key == VK_LEFT => current - STEP,
            key if key == VK_RIGHT => current + STEP,
            key if key == VK_PRIOR => current - 10,
            key if key == VK_NEXT => current + 10,
            key if key == VK_HOME => 0,
            key if key == VK_END => 100,
            _ => return,
        };
        self.set_setting(id, value);
    }

    pub unsafe fn settings_tab(&mut self, reverse: bool) {
        self.move_focus(if reverse { -1 } else { 1 });
    }
}
