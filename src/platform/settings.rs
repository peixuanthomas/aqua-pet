use super::*;

const ORDER: [usize; 3] = [WALK_CHANCE, DISTANCE_BIAS, DANCE_TO_MUSIC];
const DANCE_ROW_HEIGHT: f64 = 48.0;

pub(super) struct Row {
    pub(super) bounds: RECT,
    slider: Option<RECT>,
}

impl App {
    pub unsafe fn create_settings_panel(&mut self) {
        self.update_settings_font();
        self.layout_settings();
    }

    pub unsafe fn update_settings_font(&mut self) {
        self.settings_fonts = ui::Fonts::new(self.settings);
    }

    pub unsafe fn layout_settings(&mut self) {
        let mut client = RECT::default();
        let _ = GetClientRect(self.settings, &mut client);
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let height = client.bottom as f64 / dpi;
        let row = ((height - 144.0 - DANCE_ROW_HEIGHT) / 2.0).max(112.0);
        self.settings_regions.clear();
        for (index, id) in ORDER.into_iter().enumerate() {
            let y = 88.0 + index as f64 * row;
            let is_slider = id != DANCE_TO_MUSIC;
            self.settings_regions.insert(
                id,
                Row {
                    bounds: ui::rect(
                        dpi,
                        24.0,
                        y,
                        width - 48.0,
                        if is_slider { row } else { DANCE_ROW_HEIGHT },
                    ),
                    slider: is_slider.then(|| ui::rect(dpi, 58.0, y + 50.0, width - 116.0, 32.0)),
                },
            );
        }
        let _ = InvalidateRect(Some(self.settings), None, false);
    }

    pub unsafe fn show_settings(&mut self) {
        if !IsWindowVisible(self.settings).as_bool() {
            let work = self.work_area();
            let dpi = GetDpiForWindow(self.settings).max(96) as i32;
            let width = 500 * dpi / 96;
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
        self.settings_ui.focus = Some(WALK_CHANCE);
        self.settings_ui.hot = None;
        self.layout_settings();
    }

    pub unsafe fn paint_settings(&self, dc: HDC, client: RECT) {
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let height = client.bottom as f64 / dpi;
        ui::logo(dc, dpi, 28.0, 28.0);
        ui::label(
            dc,
            ui::rect(dpi, 144.0, 24.0, width - 172.0, 40.0),
            "桌宠设置",
            self.settings_fonts.heading,
            ui::WHITE,
        );
        ui::border(
            dc,
            ui::rect(dpi, 24.0, 88.0, width - 48.0, height - 144.0),
            ui::WHITE,
            (2.0 * dpi).round() as i32,
        );
        for id in [DISTANCE_BIAS, DANCE_TO_MUSIC] {
            let y = self.settings_regions[&id].bounds.top as f64 / dpi;
            ui::fill(dc, &ui::rect(dpi, 42.0, y, width - 84.0, 2.0), ui::DIM);
        }
        for id in [WALK_CHANCE, DISTANCE_BIAS] {
            let y = self.settings_regions[&id].bounds.top as f64 / dpi;
            let (name, value, ends) = if id == WALK_CHANCE {
                (
                    "主动走跳",
                    format!("{}%", self.wander.probability()),
                    ("静止", "经常"),
                )
            } else {
                (
                    "散步距离",
                    match self.wander.distance_bias() {
                        0..50 => "偏短".to_string(),
                        50 => "均衡".to_string(),
                        _ => "偏长".to_string(),
                    },
                    ("短", "长"),
                )
            };
            if self.settings_ui.selected(id) {
                ui::heart(dc, dpi, 38.0, y + 19.0);
            }
            ui::label(
                dc,
                ui::rect(dpi, 58.0, y + 10.0, width - 220.0, 32.0),
                name,
                self.settings_fonts.body,
                if self.settings_ui.hot_control() == Some(id) {
                    ui::YELLOW
                } else {
                    ui::WHITE
                },
            );
            let mut value_rect = ui::rect(dpi, width - 170.0, y + 10.0, 112.0, 32.0);
            ui::text(
                dc,
                &mut value_rect,
                &value,
                self.settings_fonts.value,
                ui::CYAN,
                DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_NOPREFIX,
            );
            ui::slider(
                dc,
                &self.settings_ui,
                dpi,
                id,
                self.settings_regions[&id].slider.unwrap(),
                self.slider_setting(id).unwrap(),
                100,
            );
            ui::label(
                dc,
                ui::rect(dpi, 58.0, y + 84.0, 100.0, 24.0),
                ends.0,
                self.settings_fonts.small,
                ui::MUTED,
            );
            let mut right = ui::rect(dpi, width - 158.0, y + 84.0, 100.0, 24.0);
            ui::text(
                dc,
                &mut right,
                ends.1,
                self.settings_fonts.small,
                ui::MUTED,
                DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_NOPREFIX,
            );
        }
        let y = self.settings_regions[&DANCE_TO_MUSIC].bounds.top as f64 / dpi;
        let color = if self.settings_ui.hot_control() == Some(DANCE_TO_MUSIC) {
            ui::YELLOW
        } else {
            ui::WHITE
        };
        if self.settings_ui.selected(DANCE_TO_MUSIC) {
            ui::heart(dc, dpi, 38.0, y + 18.0);
        }
        ui::label(
            dc,
            ui::rect(dpi, 58.0, y + 8.0, width - 154.0, 32.0),
            "随音乐舞动",
            self.settings_fonts.body,
            color,
        );
        let box_x = width - 78.0;
        let box_y = y + 14.0;
        ui::border(
            dc,
            ui::rect(dpi, box_x, box_y, 20.0, 20.0),
            color,
            (2.0 * dpi).round() as i32,
        );
        if self.dance_to_music {
            for (dx, dy) in [
                (4.0, 9.0),
                (6.0, 11.0),
                (8.0, 13.0),
                (10.0, 11.0),
                (12.0, 9.0),
                (14.0, 7.0),
            ] {
                ui::fill(
                    dc,
                    &ui::rect(dpi, box_x + dx, box_y + dy, 3.0, 3.0),
                    ui::CYAN,
                );
            }
        }
        ui::label(
            dc,
            ui::rect(dpi, 32.0, height - 42.0, width - 64.0, 24.0),
            "更改自动保存",
            self.settings_fonts.small,
            ui::MUTED,
        );
    }

    fn slider_setting(&self, id: usize) -> Option<i32> {
        match id {
            WALK_CHANCE => Some(self.wander.probability() as i32),
            DISTANCE_BIAS => Some(self.wander.distance_bias() as i32),
            _ => None,
        }
    }

    unsafe fn toggle_music_dance(&mut self) {
        self.dance_to_music = !self.dance_to_music;
        if !self.dance_to_music {
            self.music = None;
        }
        self.save_settings();
        let _ = InvalidateRect(
            Some(self.settings),
            Some(&self.settings_regions[&DANCE_TO_MUSIC].bounds),
            false,
        );
    }

    fn setting_at(&self, point: POINT) -> Option<usize> {
        self.settings_regions
            .iter()
            .find_map(|(&id, row)| ui::contains(&row.bounds, point).then_some(id))
    }

    unsafe fn set_settings_hover(&mut self, next: Option<usize>) {
        let old = self.settings_ui.hot_control();
        if old == next {
            return;
        }
        self.settings_ui.hot = next.map(ui::Hover::Control);
        for id in [old, next].into_iter().flatten() {
            let _ = InvalidateRect(
                Some(self.settings),
                Some(&self.settings_regions[&id].bounds),
                false,
            );
        }
    }

    fn set_setting(&mut self, id: usize, value: i32, save: bool) {
        let value = value.clamp(0, 100) as u8;
        match id {
            WALK_CHANCE if self.wander.probability() != value => self.wander.set_probability(value),
            DISTANCE_BIAS if self.wander.distance_bias() != value => {
                self.wander.set_distance_bias(value)
            }
            _ => return,
        }
        if save {
            self.save_settings();
        }
        unsafe {
            let _ = InvalidateRect(
                Some(self.settings),
                Some(&self.settings_regions[&id].bounds),
                false,
            );
        }
    }

    fn set_setting_from_point(&mut self, id: usize, point: POINT) {
        if let Some(slider) = self.settings_regions[&id].slider {
            let value = ui::slider_value(&slider, point.x, 100);
            self.set_setting(id, value, false);
        }
    }

    pub unsafe fn settings_mouse(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) {
        let point = ui::point(lp);
        match msg {
            WM_MOUSEMOVE => {
                self.settings_ui.track_pointer(self.settings);
                self.set_settings_hover(self.setting_at(point));
                if let Some(id) = self.settings_ui.active {
                    self.set_setting_from_point(id, point);
                }
            }
            WM_MOUSELEAVE if self.settings_ui.pointer_left(self.settings) => {
                self.set_settings_hover(None)
            }
            WM_LBUTTONDOWN => {
                if let Some(id) = self.setting_at(point) {
                    self.settings_ui.focus = Some(id);
                    let _ = SetFocus(Some(self.settings));
                    if self.settings_regions[&id]
                        .slider
                        .is_none_or(|slider| ui::contains(&slider, point))
                    {
                        self.settings_ui.active = Some(id);
                        SetCapture(self.settings);
                        self.set_setting_from_point(id, point);
                    }
                    let _ = InvalidateRect(Some(self.settings), None, false);
                }
            }
            WM_LBUTTONUP => {
                if let Some(id) = self.settings_ui.active.take() {
                    let _ = ReleaseCapture();
                    if id == DANCE_TO_MUSIC {
                        if self.setting_at(point) == Some(id) {
                            self.toggle_music_dance();
                        }
                    } else {
                        self.save_settings();
                    }
                }
            }
            WM_MOUSEWHEEL => {
                let delta = (wp.0 >> 16) as u16 as i16 as i32 / 120;
                if let Some(id) = self.settings_ui.hot_control().or(self.settings_ui.focus)
                    && let Some(current) = self.slider_setting(id)
                {
                    self.set_setting(id, current + delta, true);
                }
            }
            _ => {}
        }
    }

    pub unsafe fn settings_key(&mut self, key: VIRTUAL_KEY) {
        if key == VK_ESCAPE {
            self.save_settings();
            let _ = ShowWindow(self.settings, SW_HIDE);
            return;
        }
        let Some(id) = self.settings_ui.focus else {
            return;
        };
        if id == DANCE_TO_MUSIC && key == VK_SPACE {
            self.toggle_music_dance();
            return;
        }
        let Some(current) = self.slider_setting(id) else {
            return;
        };
        let value = match key {
            key if key == VK_LEFT || key == VK_DOWN => current - 1,
            key if key == VK_RIGHT || key == VK_UP => current + 1,
            key if key == VK_PRIOR => current - 10,
            key if key == VK_NEXT => current + 10,
            key if key == VK_HOME => 0,
            key if key == VK_END => 100,
            _ => return,
        };
        self.set_setting(id, value, true);
    }

    pub unsafe fn settings_tab(&mut self, reverse: bool) {
        let current = ORDER
            .iter()
            .position(|id| Some(*id) == self.settings_ui.focus)
            .unwrap_or(0) as i32;
        let next = (current + if reverse { -1 } else { 1 }).rem_euclid(ORDER.len() as i32);
        self.settings_ui.focus = Some(ORDER[next as usize]);
        let _ = SetFocus(Some(self.settings));
        let _ = InvalidateRect(Some(self.settings), None, false);
    }
}
