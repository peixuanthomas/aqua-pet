use super::*;

impl App {
    pub unsafe fn create_debug_controls(&mut self) -> windows::core::Result<()> {
        let module = GetModuleHandleW(None)?;
        self.search = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_TABSTOP.0),
            0,
            0,
            10,
            10,
            Some(self.debug),
            Some(HMENU(SEARCH as *mut c_void)),
            Some(module.into()),
            None,
        )?;
        let prompt = wide("搜索动作或资源名");
        send(self.search, 0x1501, 1, prompt.as_ptr() as isize);
        let mut categories: Vec<_> = self
            .catalog
            .clips
            .iter()
            .map(|clip| clip.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories.insert(0, "全部分类".into());
        self.choices = HashMap::from([
            (
                KIND,
                ["全部类型", "原作动作", "原始素材", "未使用素材"]
                    .map(str::to_owned)
                    .to_vec(),
            ),
            (CATEGORY, categories),
            (SPEED, RATES.iter().map(|rate| format!("{rate}×")).collect()),
            (SCALE, (1..=8).map(|scale| format!("{scale}×")).collect()),
            (BG, ["深色", "浅色", "棋盘格"].map(str::to_owned).to_vec()),
        ]);
        self.update_font();
        self.filter();
        self.sync_selection();
        self.layout();
        Ok(())
    }

    pub unsafe fn update_font(&mut self) {
        self.fonts = ui::Fonts::new(self.debug);
        send(self.search, WM_SETFONT, self.fonts.body.0 as usize, 1);
    }

    pub unsafe fn layout(&mut self) {
        let mut client = RECT::default();
        let _ = GetClientRect(self.debug, &mut client);
        let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let height = client.bottom as f64 / dpi;
        let left = 300.0_f64.min(width * 0.30);
        let right_x = left + 48.0;
        let right_width = width - right_x - 24.0;
        self.debug_regions.clear();
        let mut place = |id, x, y, w, h| {
            self.debug_regions.insert(id, ui::rect(dpi, x, y, w, h));
        };
        place(SEARCH, 24.0, 82.0, left, 36.0);
        let filter_width = (left - 8.0) / 2.0;
        place(KIND, 24.0, 134.0, filter_width, 32.0);
        place(CATEGORY, 32.0 + filter_width, 134.0, filter_width, 32.0);
        place(LIST, 24.0, 182.0, left, height - 240.0);
        place(IDLE, width - 208.0, 84.0, 88.0, 32.0);
        place(DETAILS, width - 112.0, 84.0, 88.0, 32.0);
        self.preview_rect = ui::rect(dpi, right_x, 128.0, right_width, height - 336.0);
        let bottom = height - 208.0;
        place(TIMELINE, right_x, bottom + 10.0, right_width, 28.0);
        let button_width = (right_width - 48.0) / 4.0;
        for (index, id) in [PLAY, RESTART, PREV, NEXT].iter().enumerate() {
            place(
                *id,
                right_x + index as f64 * (button_width + 16.0) + 18.0,
                bottom + 80.0,
                button_width - 18.0,
                36.0,
            );
        }
        let choice_width = (right_width - 48.0) / 4.0;
        for (index, id) in [LOOP, SPEED, SCALE, BG].iter().enumerate() {
            place(
                *id,
                right_x + index as f64 * (choice_width + 16.0) + 18.0,
                bottom + 130.0,
                choice_width - 18.0,
                32.0,
            );
        }
        if self.visible.is_empty() {
            place(CLEAR_FILTERS, 48.0, 292.0, left - 48.0, 36.0);
        }
        if self.details_open {
            place(DETAILS_CLOSE, width - 136.0, 88.0, 88.0, 32.0);
        }
        if !self.search.is_invalid() {
            let search = self.debug_regions[&SEARCH];
            let _ = MoveWindow(
                self.search,
                search.left + (8.0 * dpi) as i32,
                search.top + (3.0 * dpi) as i32,
                search.right - search.left - (16.0 * dpi) as i32,
                search.bottom - search.top - (6.0 * dpi) as i32,
                true,
            );
        }
        self.debug_ui.popup = None;
        self.refresh_debug_hover();
        self.preview_dirty = true;
        let _ = InvalidateRect(Some(self.debug), None, false);
    }

    pub unsafe fn filter(&mut self) {
        let search = window_text(self.search).to_lowercase();
        self.visible = self
            .catalog
            .clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| {
                let kind_ok = match self.kind_filter {
                    1 => clip.kind == "action",
                    2 => clip.kind == "resource",
                    3 => clip.category.contains("Unused"),
                    _ => true,
                };
                kind_ok
                    && (self.category_filter == 0
                        || self.choices[&CATEGORY].get(self.category_filter)
                            == Some(&clip.category))
                    && (search.is_empty()
                        || format!(
                            "{} {} {} {}",
                            clip.id, clip.label, clip.category, clip.notes
                        )
                        .to_lowercase()
                        .contains(&search)
                        || clip
                            .steps
                            .iter()
                            .flat_map(|step| &step.layers)
                            .any(|layer| layer.sprite.to_lowercase().contains(&search)))
            })
            .map(|(index, _)| index)
            .collect();
        self.debug_ui.list_top = 0;
        self.layout();
    }

    fn list_rows(&self) -> usize {
        let Some(rect) = self.debug_regions.get(&LIST) else {
            return 1;
        };
        let dpi = unsafe { GetDpiForWindow(self.debug).max(96) as f64 / 96.0 };
        ((rect.bottom - rect.top) / (36.0 * dpi).round() as i32).max(1) as usize
    }

    pub unsafe fn sync_selection(&mut self) {
        if let Some(position) = self
            .visible
            .iter()
            .position(|&index| index == self.player.selected)
        {
            let rows = self.list_rows();
            if position < self.debug_ui.list_top {
                self.debug_ui.list_top = position;
            } else if position >= self.debug_ui.list_top + rows {
                self.debug_ui.list_top = position + 1 - rows;
            }
        }
        self.details_scroll = 0;
        self.last_status = Instant::now() - Duration::from_secs(1);
        let _ = InvalidateRect(Some(self.debug), None, false);
    }

    fn choice_items(&self, id: usize) -> &[String] {
        &self.choices[&id]
    }

    fn choice_index(&self, id: usize) -> usize {
        match id {
            KIND => self.kind_filter,
            CATEGORY => self.category_filter,
            SPEED => RATES
                .iter()
                .position(|rate| *rate == self.player.rate)
                .unwrap_or(3),
            SCALE => (self.scale - 1) as usize,
            BG => self.bg,
            _ => 0,
        }
    }

    fn popup_rect(&self, id: usize) -> RECT {
        let anchor = &self.debug_regions[&id];
        let dpi = unsafe { GetDpiForWindow(self.debug).max(96) as f64 / 96.0 };
        let mut rect = ui::popup_rect(anchor, dpi, self.choice_items(id).len());
        let mut client = RECT::default();
        unsafe {
            let _ = GetClientRect(self.debug, &mut client);
        }
        if rect.bottom > client.bottom {
            let height = rect.bottom - rect.top;
            rect.top = anchor.top - height - (4.0 * dpi) as i32;
            rect.bottom = rect.top + height;
        }
        rect
    }

    fn popup_row(&self, point: POINT) -> Option<usize> {
        let id = self.debug_ui.popup?;
        let rect = self.popup_rect(id);
        if !ui::contains(&rect, point) {
            return None;
        }
        let dpi = unsafe { GetDpiForWindow(self.debug).max(96) as f64 / 96.0 };
        let y = point.y - rect.top - (2.0 * dpi) as i32;
        let row = usize::try_from(y).ok()? / (34.0 * dpi).round() as usize;
        let index = self.debug_ui.popup_top + row;
        (row < 8 && index < self.choice_items(id).len()).then_some(index)
    }

    fn list_row(&self, point: POINT) -> Option<usize> {
        let rect = self.debug_regions.get(&LIST)?;
        if !ui::contains(rect, point) {
            return None;
        }
        let dpi = unsafe { GetDpiForWindow(self.debug).max(96) as f64 / 96.0 };
        let row = (point.y - rect.top) as usize / (36.0 * dpi).round() as usize;
        let index = self.debug_ui.list_top + row;
        (row < self.list_rows() && index < self.visible.len()).then_some(index)
    }

    fn details_panel(&self) -> RECT {
        let dpi = unsafe { GetDpiForWindow(self.debug).max(96) as f64 / 96.0 };
        let mut client = RECT::default();
        unsafe {
            let _ = GetClientRect(self.debug, &mut client);
        }
        RECT {
            left: self.preview_rect.left,
            top: (76.0 * dpi) as i32,
            right: self.preview_rect.right,
            bottom: client.bottom - (24.0 * dpi) as i32,
        }
    }

    fn debug_hit(&self, point: POINT) -> Option<usize> {
        if self.details_open && ui::contains(&self.details_panel(), point) {
            return self
                .debug_regions
                .get(&DETAILS_CLOSE)
                .filter(|rect| ui::contains(rect, point))
                .map(|_| DETAILS_CLOSE);
        }
        if self
            .debug_regions
            .get(&CLEAR_FILTERS)
            .is_some_and(|rect| ui::contains(rect, point))
        {
            return Some(CLEAR_FILTERS);
        }
        self.debug_regions
            .iter()
            .find_map(|(&id, rect)| ui::contains(rect, point).then_some(id))
    }

    fn debug_hover_at(&self, point: POINT) -> Option<ui::Hover> {
        if self.debug_ui.popup.is_some() {
            return self.popup_row(point).map(ui::Hover::PopupRow);
        }
        match self.debug_hit(point)? {
            LIST => self.list_row(point).map(ui::Hover::ActionRow),
            id => Some(ui::Hover::Control(id)),
        }
    }

    unsafe fn debug_hover_area(&self, hot: ui::Hover) -> Option<RECT> {
        let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
        let (mut area, row, height) = match hot {
            ui::Hover::Control(id) => return self.debug_regions.get(&id).copied(),
            ui::Hover::ActionRow(index) => (
                self.debug_regions[&LIST],
                index.checked_sub(self.debug_ui.list_top)?,
                36.0,
            ),
            ui::Hover::PopupRow(index) => {
                let mut rect = self.popup_rect(self.debug_ui.popup?);
                rect.top += (2.0 * dpi) as i32;
                (rect, index.checked_sub(self.debug_ui.popup_top)?, 34.0)
            }
        };
        let height = (height * dpi).round() as i32;
        area.top += row as i32 * height;
        area.bottom = area.bottom.min(area.top + height);
        (area.top < area.bottom).then_some(area)
    }

    unsafe fn set_debug_hover(&mut self, next: Option<ui::Hover>) {
        let old = self.debug_ui.hot;
        if next == old {
            return;
        }
        self.debug_ui.hot = next;
        for hot in [old, next].into_iter().flatten() {
            if let Some(rect) = self.debug_hover_area(hot) {
                let _ = InvalidateRect(Some(self.debug), Some(&rect), false);
            }
        }
    }

    unsafe fn refresh_debug_hover(&mut self) {
        let mut point = POINT::default();
        let window = GetCursorPos(&mut point)
            .ok()
            .map(|()| WindowFromPoint(point));
        let next = if let Some(hwnd) = window
            && (hwnd == self.debug || hwnd == self.search)
        {
            self.debug_ui.track_pointer(hwnd);
            let _ = ScreenToClient(self.debug, &mut point);
            self.debug_hover_at(point)
        } else {
            None
        };
        self.set_debug_hover(next);
    }

    pub(super) unsafe fn debug_pointer(&mut self, hwnd: HWND, msg: u32, point: POINT) {
        if msg == WM_MOUSEMOVE {
            self.debug_ui.track_pointer(hwnd);
            self.set_debug_hover(self.debug_hover_at(point));
        } else if self.debug_ui.pointer_left(hwnd) {
            self.set_debug_hover(None);
        }
    }

    fn toggle_details(&mut self, open: bool) {
        self.details_open = open;
        self.details_scroll = 0;
        self.debug_ui.focus = Some(if open { DETAILS_CLOSE } else { DETAILS });
        unsafe {
            self.layout();
        }
    }

    fn activate(&mut self, id: usize) {
        let clip = &self.catalog.clips[self.player.selected];
        match id {
            PLAY => self.player.toggle(),
            RESTART => self.player.restart(),
            PREV => self.player.step(clip, false),
            NEXT => self.player.step(clip, true),
            LOOP => self.player.looping = !self.player.looping,
            IDLE => self.select(self.catalog.idle_index()),
            DETAILS => {
                self.toggle_details(true);
                return;
            }
            DETAILS_CLOSE => {
                self.toggle_details(false);
                return;
            }
            CLEAR_FILTERS => unsafe {
                text(self.search, "");
                self.kind_filter = 0;
                self.category_filter = 0;
                self.debug_ui.focus = Some(LIST);
                self.filter();
                return;
            },
            KIND | CATEGORY | SPEED | SCALE | BG => {
                if self.debug_ui.popup == Some(id) {
                    self.debug_ui.popup = None;
                } else {
                    self.debug_ui.popup = Some(id);
                    self.debug_ui.popup_top = self.choice_index(id).saturating_sub(3);
                    let maximum = self.choice_items(id).len().saturating_sub(8);
                    self.debug_ui.popup_top = self.debug_ui.popup_top.min(maximum);
                }
                unsafe {
                    self.refresh_debug_hover();
                    let _ = InvalidateRect(Some(self.debug), None, false);
                }
                return;
            }
            _ => return,
        }
        self.preview_dirty = true;
        self.last_status = Instant::now() - Duration::from_secs(1);
        self.last_clock = Instant::now();
        unsafe {
            let _ = InvalidateRect(Some(self.debug), None, false);
        }
    }

    fn select_choice(&mut self, id: usize, index: usize) {
        match id {
            KIND => {
                self.kind_filter = index;
                unsafe { self.filter() };
            }
            CATEGORY => {
                self.category_filter = index;
                unsafe { self.filter() };
            }
            SPEED => self.player.rate = RATES[index.min(RATES.len() - 1)],
            SCALE => {
                self.scale = index as u32 + 1;
                unsafe { self.fit(None) };
                self.last_frame = None;
                self.save_settings();
            }
            BG => self.bg = index.min(2),
            _ => return,
        }
        self.preview_dirty = true;
        self.last_status = Instant::now() - Duration::from_secs(1);
        self.last_clock = Instant::now();
        unsafe {
            let _ = InvalidateRect(Some(self.debug), None, false);
        }
    }

    fn seek_from_point(&mut self, point: POINT) {
        let Some(rect) = self.debug_regions.get(&TIMELINE) else {
            return;
        };
        let position = ui::slider_value(rect, point.x, 10000);
        let clip = &self.catalog.clips[self.player.selected];
        self.seek_debug(clip.duration() * position as f64 / 10000.0);
    }

    fn seek_debug(&mut self, tick: f64) {
        let clip = &self.catalog.clips[self.player.selected];
        if self.player.playing || self.player.waiting.is_some() || self.player.tick != tick {
            self.player.seek(clip, tick);
            unsafe { self.invalidate_playback() };
        }
    }

    pub unsafe fn debug_command(&mut self, wp: WPARAM) {
        let id = wp.0 & 0xffff;
        let notification = (wp.0 >> 16) & 0xffff;
        if id == SEARCH && notification == EN_CHANGE as usize {
            self.filter();
        } else if id == SEARCH && notification == EN_SETFOCUS as usize {
            self.debug_ui.popup = None;
            self.debug_ui.focus = Some(SEARCH);
            self.refresh_debug_hover();
            let _ = InvalidateRect(Some(self.debug), None, false);
        }
    }

    pub unsafe fn debug_mouse(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) {
        let point = ui::point(lp);
        match msg {
            WM_MOUSEMOVE | WM_MOUSELEAVE => {
                self.debug_pointer(self.debug, msg, point);
                if msg == WM_MOUSEMOVE && self.debug_ui.active == Some(TIMELINE) {
                    self.seek_from_point(point);
                }
            }
            WM_LBUTTONDOWN => {
                if let Some(popup) = self.debug_ui.popup {
                    if let Some(index) = self.popup_row(point) {
                        self.debug_ui.popup = None;
                        self.select_choice(popup, index);
                    } else {
                        self.debug_ui.popup = None;
                        let _ = InvalidateRect(Some(self.debug), None, false);
                    }
                    self.set_debug_hover(self.debug_hover_at(point));
                    return;
                }
                let Some(id) = self.debug_hit(point) else {
                    return;
                };
                self.debug_ui.focus = Some(id);
                let _ = InvalidateRect(Some(self.debug), None, false);
                if id == SEARCH {
                    let _ = SetFocus(Some(self.search));
                    return;
                }
                let _ = SetFocus(Some(self.debug));
                self.debug_ui.active = Some(id);
                SetCapture(self.debug);
                if id == TIMELINE {
                    self.seek_from_point(point);
                } else if id == LIST
                    && let Some(row) = self.list_row(point)
                {
                    self.select(self.visible[row]);
                }
            }
            WM_LBUTTONUP => {
                if let Some(id) = self.debug_ui.active.take() {
                    let _ = ReleaseCapture();
                    if self.debug_hit(point) == Some(id) && id != LIST && id != TIMELINE {
                        self.activate(id);
                    }
                    let _ = InvalidateRect(Some(self.debug), None, false);
                }
            }
            WM_MOUSEWHEEL => {
                let delta = (wp.0 >> 16) as u16 as i16 as i32;
                let direction = if delta > 0 { -3 } else { 3 };
                let area = if let Some(popup) = self.debug_ui.popup {
                    let count = self.choice_items(popup).len();
                    self.debug_ui.popup_top =
                        ui::scroll_offset(self.debug_ui.popup_top, direction, count, 8);
                    self.popup_rect(popup)
                } else if self.details_open {
                    let panel = self.details_panel();
                    let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
                    let visible = (panel.bottom - panel.top - (86.0 * dpi) as i32).max(1);
                    let maximum = (self.details_height - visible).max(0);
                    self.details_scroll =
                        (self.details_scroll - delta.signum() * 48).clamp(0, maximum);
                    panel
                } else {
                    let count = self.visible.len();
                    self.debug_ui.list_top = ui::scroll_offset(
                        self.debug_ui.list_top,
                        direction,
                        count,
                        self.list_rows(),
                    );
                    self.debug_regions[&LIST]
                };
                self.refresh_debug_hover();
                let _ = InvalidateRect(Some(self.debug), Some(&area), false);
            }
            _ => {}
        }
    }

    fn select_visible(&mut self, position: usize) {
        if self.visible.is_empty() {
            return;
        }
        let position = position.min(self.visible.len() - 1);
        self.select(self.visible[position]);
    }

    pub unsafe fn debug_key(&mut self, key: VIRTUAL_KEY) {
        if key == VK_ESCAPE {
            if self.debug_ui.popup.take().is_some() {
                self.refresh_debug_hover();
                let _ = InvalidateRect(Some(self.debug), None, false);
            } else if self.details_open {
                self.toggle_details(false);
            } else {
                self.close_debug();
            }
            return;
        }
        if let Some(popup) = self.debug_ui.popup {
            let count = self.choice_items(popup).len();
            let current = self.choice_index(popup) as i32;
            let next = match key {
                key if key == VK_UP || key == VK_LEFT => current - 1,
                key if key == VK_DOWN || key == VK_RIGHT => current + 1,
                key if key == VK_HOME => 0,
                key if key == VK_END => count as i32 - 1,
                key if key == VK_RETURN || key == VK_SPACE => {
                    self.debug_ui.popup = None;
                    self.refresh_debug_hover();
                    let _ = InvalidateRect(Some(self.debug), None, false);
                    return;
                }
                _ => return,
            };
            let index = next.clamp(0, count as i32 - 1) as usize;
            self.select_choice(popup, index);
            self.debug_ui.popup = Some(popup);
            self.debug_ui.popup_top = self
                .debug_ui
                .popup_top
                .clamp(index.saturating_sub(7), index);
            self.refresh_debug_hover();
            return;
        }
        let Some(focus) = self.debug_ui.focus else {
            return;
        };
        if focus == LIST {
            let current = self
                .visible
                .iter()
                .position(|&index| index == self.player.selected)
                .unwrap_or(self.debug_ui.list_top) as i32;
            let next = match key {
                key if key == VK_UP => current - 1,
                key if key == VK_DOWN => current + 1,
                key if key == VK_PRIOR => current - self.list_rows() as i32,
                key if key == VK_NEXT => current + self.list_rows() as i32,
                key if key == VK_HOME => 0,
                key if key == VK_END => self.visible.len() as i32 - 1,
                _ => return,
            };
            self.select_visible(next.max(0) as usize);
            return;
        }
        if focus == TIMELINE {
            let clip = &self.catalog.clips[self.player.selected];
            let next = match key {
                key if key == VK_LEFT || key == VK_DOWN => self.player.tick - 1.0,
                key if key == VK_RIGHT || key == VK_UP => self.player.tick + 1.0,
                key if key == VK_PRIOR => self.player.tick - 30.0,
                key if key == VK_NEXT => self.player.tick + 30.0,
                key if key == VK_HOME => 0.0,
                key if key == VK_END => clip.duration(),
                _ => return,
            };
            self.seek_debug(next);
            return;
        }
        if matches!(focus, KIND | CATEGORY | SPEED | SCALE | BG)
            && matches!(key, key if key == VK_LEFT || key == VK_RIGHT || key == VK_UP || key == VK_DOWN)
        {
            let count = self.choice_items(focus).len();
            let current = self.choice_index(focus) as i32;
            let backwards = key == VK_LEFT || key == VK_UP;
            let next = (current + if backwards { -1 } else { 1 }).clamp(0, count as i32 - 1);
            self.select_choice(focus, next as usize);
            return;
        }
        if key == VK_RETURN || key == VK_SPACE {
            self.activate(focus);
        }
    }

    pub unsafe fn debug_tab(&mut self, reverse: bool) {
        let list = if self.visible.is_empty() {
            CLEAR_FILTERS
        } else {
            LIST
        };
        let order: &[usize] = if self.details_open {
            &[SEARCH, KIND, CATEGORY, list, DETAILS_CLOSE]
        } else {
            &[
                SEARCH, KIND, CATEGORY, list, PLAY, RESTART, PREV, NEXT, IDLE, DETAILS, LOOP,
                SPEED, SCALE, BG, TIMELINE,
            ]
        };
        let current = order
            .iter()
            .position(|id| Some(*id) == self.debug_ui.focus)
            .unwrap_or(0) as i32;
        let next = (current + if reverse { -1 } else { 1 }).rem_euclid(order.len() as i32);
        let id = order[next as usize];
        self.debug_ui.popup = None;
        self.debug_ui.focus = Some(id);
        if id == SEARCH {
            let _ = SetFocus(Some(self.search));
        } else {
            let _ = SetFocus(Some(self.debug));
        }
        self.refresh_debug_hover();
        let _ = InvalidateRect(Some(self.debug), None, false);
    }

    fn debug_region(&self, id: usize) -> ui::ControlRegion {
        ui::ControlRegion {
            id,
            rect: self.debug_regions[&id],
        }
    }

    pub(super) unsafe fn invalidate_playback(&self) {
        let area = RECT {
            bottom: self.debug_regions[&PLAY].bottom,
            ..self.debug_regions[&TIMELINE]
        };
        let _ = InvalidateRect(Some(self.debug), Some(&area), false);
    }

    pub unsafe fn paint(&mut self, dc: HDC, client: RECT) {
        let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
        let width = client.right as f64 / dpi;
        let left = 300.0_f64.min(width * 0.30);
        let right_x = left + 48.0;
        let right_width = width - right_x - 24.0;
        ui::logo(dc, dpi, 24.0, 26.0);
        ui::label(
            dc,
            ui::rect(dpi, 140.0, 22.0, width - 164.0, 40.0),
            "动作预览",
            self.fonts.heading,
            ui::WHITE,
        );
        if self.debug_ui.selected(SEARCH) {
            ui::heart(dc, dpi, 8.0, 94.0);
        }
        ui::border(
            dc,
            self.debug_regions[&SEARCH],
            if self.debug_ui.hot_control() == Some(SEARCH) {
                ui::YELLOW
            } else if self.debug_ui.selected(SEARCH) {
                ui::CYAN
            } else {
                ui::WHITE
            },
            (2.0 * dpi).round() as i32,
        );
        for id in [KIND, CATEGORY] {
            let name = if id == KIND { "类型" } else { "分类" };
            let items = self.choice_items(id);
            ui::choice(
                dc,
                &self.debug_ui,
                dpi,
                self.debug_region(id),
                name,
                &items[self.choice_index(id)],
                self.fonts.small,
            );
        }
        self.paint_list(dc, dpi);
        let clip = &self.catalog.clips[self.player.selected];
        ui::label(
            dc,
            ui::rect(dpi, right_x, 82.0, right_width - 200.0, 36.0),
            &clip.label,
            self.fonts.body,
            ui::YELLOW,
        );
        for id in [IDLE, DETAILS] {
            ui::button(
                dc,
                &self.debug_ui,
                dpi,
                self.debug_region(id),
                if id == IDLE {
                    "默认动作"
                } else {
                    "动作信息"
                },
                self.fonts.small,
                false,
            );
        }
        if !self.details_open && RectVisible(dc, &self.preview_rect).as_bool() {
            self.paint_preview(dc);
        }
        let position = (self.player.tick / clip.duration() * 10000.0).round() as i32;
        let timeline = self.debug_regions[&TIMELINE];
        if self.debug_ui.selected(TIMELINE) {
            ui::heart(
                dc,
                dpi,
                timeline.left as f64 / dpi - 18.0,
                timeline.top as f64 / dpi + 8.0,
            );
        }
        ui::slider(dc, &self.debug_ui, dpi, TIMELINE, timeline, position, 10000);
        let index = self.player.index(clip);
        let state = if self.player.waiting.is_some() {
            "等待继续"
        } else if self.player.finished {
            "播放完成"
        } else if self.player.playing {
            "播放中"
        } else {
            "已暂停"
        };
        ui::label(
            dc,
            ui::rect(
                dpi,
                right_x,
                self.preview_rect.bottom as f64 / dpi + 44.0,
                right_width,
                26.0,
            ),
            &format!(
                "{}   {:.2} / {:.2} 秒   样本 {}/{}   图帧 {}",
                state,
                self.player.tick / 30.0,
                clip.duration() / 30.0,
                index + 1,
                clip.steps.len(),
                clip.steps[index]
                    .layers
                    .last()
                    .map_or("空白".into(), |layer| (layer.frame + 1).to_string())
            ),
            self.fonts.small,
            ui::MUTED,
        );
        let play_label = if self.player.waiting.is_some() {
            "继续"
        } else if self.player.playing {
            "暂停"
        } else {
            "播放"
        };
        for id in [PLAY, RESTART, PREV, NEXT] {
            ui::button(
                dc,
                &self.debug_ui,
                dpi,
                self.debug_region(id),
                match id {
                    PLAY => play_label,
                    RESTART => "重播",
                    PREV => "上一帧",
                    _ => "下一帧",
                },
                self.fonts.body,
                id == PLAY,
            );
        }
        ui::button(
            dc,
            &self.debug_ui,
            dpi,
            self.debug_region(LOOP),
            if self.player.looping {
                "循环：开"
            } else {
                "循环：关"
            },
            self.fonts.small,
            self.player.looping,
        );
        for (id, name) in [(SPEED, "速度"), (SCALE, "大小"), (BG, "背景")] {
            let items = self.choice_items(id);
            ui::choice(
                dc,
                &self.debug_ui,
                dpi,
                self.debug_region(id),
                name,
                &items[self.choice_index(id)],
                self.fonts.small,
            );
        }
        if self.details_open {
            self.paint_details(dc, dpi);
        }
        if let Some(popup) = self.debug_ui.popup {
            self.paint_popup(dc, dpi, popup);
        }
    }

    unsafe fn paint_preview(&self, dc: HDC) {
        let color = match self.bg {
            0 => ui::BLACK,
            1 => COLORREF(0xeaf0f0),
            _ => COLORREF(0xded8d2),
        };
        ui::fill(dc, &self.preview_rect, color);
        if let Some((info, pixels)) = &self.preview {
            let width = info.bmiHeader.biWidth;
            let height = -info.bmiHeader.biHeight;
            let x = self.preview_rect.left
                + (self.preview_rect.right - self.preview_rect.left - width) / 2;
            let y = self.preview_rect.top
                + (self.preview_rect.bottom - self.preview_rect.top - height) / 2;
            SetDIBitsToDevice(
                dc,
                x,
                y,
                width as u32,
                height as u32,
                0,
                0,
                0,
                height as u32,
                pixels.as_ptr().cast(),
                info,
                DIB_RGB_COLORS,
            );
        }
        let dpi = GetDpiForWindow(self.debug).max(96) as f64 / 96.0;
        ui::border(dc, self.preview_rect, ui::WHITE, (2.0 * dpi).round() as i32);
    }

    unsafe fn paint_list(&self, dc: HDC, dpi: f64) {
        let rect = self.debug_regions[&LIST];
        let area = RECT {
            bottom: rect.bottom + (34.0 * dpi).round() as i32,
            ..rect
        };
        if !RectVisible(dc, &area).as_bool() {
            return;
        }
        ui::border(dc, rect, ui::WHITE, (2.0 * dpi).round() as i32);
        let saved = SaveDC(dc);
        IntersectClipRect(
            dc,
            rect.left + 2,
            rect.top + 2,
            rect.right - 2,
            rect.bottom - 2,
        );
        let row_height = (36.0 * dpi).round() as i32;
        let selected = self
            .visible
            .iter()
            .position(|&index| index == self.player.selected);
        for row in 0..self
            .list_rows()
            .min(self.visible.len().saturating_sub(self.debug_ui.list_top))
        {
            let position = self.debug_ui.list_top + row;
            let clip = &self.catalog.clips[self.visible[position]];
            let y = rect.top + row as i32 * row_height;
            if self.debug_ui.selected(LIST) && selected == Some(position) {
                ui::heart(dc, dpi, 30.0, y as f64 / dpi + 12.0);
            }
            let tag = if clip.kind == "resource" {
                "[素材]"
            } else if clip.verified {
                "[原作]"
            } else {
                "[待核实]"
            };
            ui::label(
                dc,
                RECT {
                    left: rect.left + (28.0 * dpi) as i32,
                    top: y,
                    right: rect.right - (6.0 * dpi) as i32,
                    bottom: y + row_height,
                },
                &format!("{tag} {}", clip.label),
                self.fonts.small,
                if self.debug_ui.hot == Some(ui::Hover::ActionRow(position)) {
                    ui::YELLOW
                } else {
                    ui::WHITE
                },
            );
        }
        let _ = RestoreDC(dc, saved);
        if self.visible.is_empty() {
            ui::label(
                dc,
                ui::rect(
                    dpi,
                    38.0,
                    232.0,
                    300.0_f64.min(self.debug_regions[&LIST].right as f64 / dpi) - 52.0,
                    36.0,
                ),
                "没有匹配的动作",
                self.fonts.body,
                ui::MUTED,
            );
            ui::button(
                dc,
                &self.debug_ui,
                dpi,
                self.debug_region(CLEAR_FILTERS),
                "清空筛选",
                self.fonts.body,
                true,
            );
        }
        ui::label(
            dc,
            RECT {
                left: rect.left,
                top: rect.bottom + (10.0 * dpi) as i32,
                right: rect.right,
                bottom: rect.bottom + (34.0 * dpi) as i32,
            },
            &format!("{} 个结果", self.visible.len()),
            self.fonts.small,
            ui::MUTED,
        );
    }

    fn details_text(&self) -> String {
        let clip = &self.catalog.clips[self.player.selected];
        let resources = clip
            .steps
            .iter()
            .flat_map(|step| &step.layers)
            .map(|layer| layer.sprite.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\r\n");
        format!(
            "类型\r\n{}\r\n\r\n分类\r\n{}\r\n\r\n原始资源\r\n{}\r\n\r\n说明\r\n{}\r\n\r\n原作依据\r\n{}\r\n\r\ndata.win SHA-256\r\n{}",
            if clip.kind == "resource" {
                "原始素材"
            } else if clip.verified {
                "原作动作（已对照脚本）"
            } else {
                "动作（待核实）"
            },
            clip.category,
            resources,
            clip.notes,
            clip.evidence.join("\r\n"),
            self.catalog.source_hash
        )
    }

    unsafe fn paint_details(&mut self, dc: HDC, dpi: f64) {
        let panel = self.details_panel();
        if !RectVisible(dc, &panel).as_bool() {
            return;
        }
        ui::fill(dc, &panel, ui::BLACK);
        ui::border(dc, panel, ui::WHITE, (2.0 * dpi).round() as i32);
        ui::label(
            dc,
            RECT {
                left: panel.left + (24.0 * dpi) as i32,
                top: panel.top + (14.0 * dpi) as i32,
                right: panel.right - (120.0 * dpi) as i32,
                bottom: panel.top + (52.0 * dpi) as i32,
            },
            "动作信息",
            self.fonts.heading,
            ui::WHITE,
        );
        ui::button(
            dc,
            &self.debug_ui,
            dpi,
            self.debug_region(DETAILS_CLOSE),
            "关闭",
            self.fonts.small,
            false,
        );
        let viewport = RECT {
            left: panel.left + (24.0 * dpi) as i32,
            top: panel.top + (66.0 * dpi) as i32,
            right: panel.right - (24.0 * dpi) as i32,
            bottom: panel.bottom - (20.0 * dpi) as i32,
        };
        let mut measure = viewport;
        measure.bottom = measure.top;
        let details = self.details_text();
        let old = SelectObject(dc, self.fonts.body.into());
        let mut wide_details = wide(&details);
        DrawTextW(
            dc,
            &mut wide_details,
            &mut measure,
            DT_WORDBREAK | DT_NOPREFIX | DT_CALCRECT,
        );
        SelectObject(dc, old);
        self.details_height = measure.bottom - measure.top;
        let visible = viewport.bottom - viewport.top;
        self.details_scroll = self
            .details_scroll
            .clamp(0, (self.details_height - visible).max(0));
        let saved = SaveDC(dc);
        IntersectClipRect(
            dc,
            viewport.left,
            viewport.top,
            viewport.right,
            viewport.bottom,
        );
        let mut content = RECT {
            top: viewport.top - self.details_scroll,
            bottom: viewport.top - self.details_scroll + self.details_height,
            ..viewport
        };
        ui::text(
            dc,
            &mut content,
            &details,
            self.fonts.body,
            ui::WHITE,
            DT_WORDBREAK | DT_NOPREFIX,
        );
        let _ = RestoreDC(dc, saved);
        if self.details_height > visible {
            let track = RECT {
                left: panel.right - (8.0 * dpi) as i32,
                top: viewport.top,
                right: panel.right - (6.0 * dpi) as i32,
                bottom: viewport.bottom,
            };
            ui::fill(dc, &track, ui::DIM);
            let thumb_height = (visible * visible / self.details_height).max(12);
            let thumb_top = track.top
                + (track.bottom - track.top - thumb_height) * self.details_scroll
                    / (self.details_height - visible).max(1);
            ui::fill(
                dc,
                &RECT {
                    top: thumb_top,
                    bottom: thumb_top + thumb_height,
                    ..track
                },
                ui::CYAN,
            );
        }
    }

    unsafe fn paint_popup(&self, dc: HDC, dpi: f64, id: usize) {
        let items = self.choice_items(id);
        let rect = self.popup_rect(id);
        ui::fill(dc, &rect, ui::BLACK);
        ui::border(dc, rect, ui::WHITE, (2.0 * dpi).round() as i32);
        let row_height = (34.0 * dpi).round() as i32;
        for row in 0..8.min(items.len().saturating_sub(self.debug_ui.popup_top)) {
            let index = self.debug_ui.popup_top + row;
            let item = RECT {
                left: rect.left + (24.0 * dpi) as i32,
                top: rect.top + (2.0 * dpi) as i32 + row as i32 * row_height,
                right: rect.right - (6.0 * dpi) as i32,
                bottom: rect.top + (2.0 * dpi) as i32 + (row + 1) as i32 * row_height,
            };
            if self.choice_index(id) == index {
                ui::heart(
                    dc,
                    dpi,
                    rect.left as f64 / dpi + 6.0,
                    item.top as f64 / dpi + 11.0,
                );
            }
            ui::label(
                dc,
                item,
                &items[index],
                self.fonts.small,
                if self.debug_ui.hot == Some(ui::Hover::PopupRow(index)) {
                    ui::YELLOW
                } else {
                    ui::WHITE
                },
            );
        }
    }
}
