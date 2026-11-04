use super::*;

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
        let row = ((height - 144.0) / 2.0).max(112.0);
        self.settings_regions.clear();
        for (index, id) in [WALK_CHANCE, DISTANCE_BIAS].iter().enumerate() {
            let y = 88.0 + index as f64 * row;
            self.settings_regions
                .insert(*id, ui::rect(dpi, 58.0, y + 50.0, width - 116.0, 32.0));
        }
        let _ = InvalidateRect(Some(self.settings), None, true);
    }

    pub unsafe fn show_settings(&mut self) {
        if !IsWindowVisible(self.settings).as_bool() {
            let work = self.work_area();
            let dpi = GetDpiForWindow(self.settings).max(96) as i32;
            let width = 500 * dpi / 96;
            let height = 440 * dpi / 96;
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
        self.layout_settings();
        let _ = ShowWindow(self.settings, SW_RESTORE);
        let _ = SetForegroundWindow(self.settings);
        let _ = SetFocus(Some(self.settings));
        self.settings_ui.focus = Some(WALK_CHANCE);
    }

    pub unsafe fn paint_settings(&self) {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(self.settings, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(self.settings, &mut client);
        ui::fill(dc, &client, ui::BLACK);
        let dpi = GetDpiForWindow(self.settings).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let height = client.bottom as f64 / dpi;
        let row = ((height - 144.0) / 2.0).max(112.0);
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
        ui::fill(
            dc,
            &ui::rect(dpi, 42.0, 88.0 + row, width - 84.0, 2.0),
            ui::DIM,
        );
        for (index, id) in [WALK_CHANCE, DISTANCE_BIAS].iter().enumerate() {
            let y = 88.0 + index as f64 * row;
            let (name, value, ends) = if *id == WALK_CHANCE {
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
            if self.settings_ui.hot == Some(*id) || self.settings_ui.focus == Some(*id) {
                ui::heart(dc, dpi, 38.0, y + 19.0);
            }
            ui::label(
                dc,
                ui::rect(dpi, 58.0, y + 10.0, width - 220.0, 32.0),
                name,
                self.settings_fonts.body,
                if self.settings_ui.focus == Some(*id) {
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
            let current = if *id == WALK_CHANCE {
                self.wander.probability()
            } else {
                self.wander.distance_bias()
            };
            ui::slider(
                dc,
                &self.settings_ui,
                dpi,
                *id,
                self.settings_regions[id],
                current as i32,
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
        ui::label(
            dc,
            ui::rect(dpi, 32.0, height - 42.0, width - 64.0, 24.0),
            "更改自动保存",
            self.settings_fonts.small,
            ui::MUTED,
        );
        let _ = EndPaint(self.settings, &ps);
    }

    fn setting_at(&self, point: POINT) -> Option<usize> {
        self.settings_regions
            .iter()
            .find_map(|(&id, rect)| ui::contains(rect, point).then_some(id))
    }

    fn set_setting(&mut self, id: usize, value: i32, save: bool) {
        let value = value.clamp(0, 100) as u8;
        match id {
            WALK_CHANCE => self.wander.set_probability(value),
            DISTANCE_BIAS => self.wander.set_distance_bias(value),
            _ => return,
        }
        if save {
            self.save_settings();
        }
        unsafe {
            let _ = InvalidateRect(Some(self.settings), None, false);
        }
    }

    fn set_setting_from_point(&mut self, id: usize, point: POINT) {
        if let Some(rect) = self.settings_regions.get(&id) {
            let value = ui::slider_value(rect, point.x, 100);
            self.set_setting(id, value, false);
        }
    }

    pub unsafe fn settings_mouse(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) {
        let point = ui::point(lp);
        match msg {
            WM_MOUSEMOVE => {
                let hot = self.setting_at(point);
                if self.settings_ui.hot != hot {
                    self.settings_ui.hot = hot;
                    let _ = InvalidateRect(Some(self.settings), None, false);
                }
                if let Some(id) = self.settings_ui.active {
                    self.set_setting_from_point(id, point);
                }
            }
            WM_LBUTTONDOWN => {
                if let Some(id) = self.setting_at(point) {
                    self.settings_ui.active = Some(id);
                    self.settings_ui.focus = Some(id);
                    let _ = SetFocus(Some(self.settings));
                    SetCapture(self.settings);
                    self.set_setting_from_point(id, point);
                }
            }
            WM_LBUTTONUP => {
                if self.settings_ui.active.take().is_some() {
                    let _ = ReleaseCapture();
                    self.save_settings();
                    let _ = InvalidateRect(Some(self.settings), None, false);
                }
            }
            WM_MOUSEWHEEL => {
                let delta = (wp.0 >> 16) as u16 as i16 as i32 / 120;
                if let Some(id) = self.settings_ui.hot.or(self.settings_ui.focus) {
                    let current = if id == WALK_CHANCE {
                        self.wander.probability()
                    } else {
                        self.wander.distance_bias()
                    };
                    self.set_setting(id, current as i32 + delta, true);
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
        let current = if id == WALK_CHANCE {
            self.wander.probability() as i32
        } else {
            self.wander.distance_bias() as i32
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
        let next = match (self.settings_ui.focus, reverse) {
            (Some(WALK_CHANCE), false) | (Some(DISTANCE_BIAS), true) => DISTANCE_BIAS,
            _ => WALK_CHANCE,
        };
        self.settings_ui.focus = Some(next);
        let _ = SetFocus(Some(self.settings));
        let _ = InvalidateRect(Some(self.settings), None, false);
    }
}
