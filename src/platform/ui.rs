use super::*;

pub const BLACK: COLORREF = COLORREF(0x000000);
pub const WHITE: COLORREF = COLORREF(0xffffff);
pub const CYAN: COLORREF = COLORREF(0xf5e469);
pub const MUTED: COLORREF = COLORREF(0xbdb6b6);
pub const RED: COLORREF = COLORREF(0x0000ff);
pub const YELLOW: COLORREF = COLORREF(0x00ffff);
pub const DIM: COLORREF = COLORREF(0x3c3c3c);

#[derive(Default)]
pub struct State {
    pub hot: Option<usize>,
    pub active: Option<usize>,
    pub focus: Option<usize>,
    pub popup: Option<usize>,
    pub popup_top: usize,
    pub popup_hot: Option<usize>,
    pub list_top: usize,
    pub list_hot: Option<usize>,
}

#[derive(Default)]
pub struct Fonts {
    pub body: HFONT,
    pub small: HFONT,
    pub heading: HFONT,
    pub value: HFONT,
}

impl Fonts {
    pub unsafe fn new(window: HWND) -> Self {
        let dpi = GetDpiForWindow(window).max(96) as i32;
        let make = |size: i32, weight, face: PCWSTR| {
            CreateFontW(
                -size * dpi / 96,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                face,
            )
        };
        Self {
            body: make(16, 400, w!("Microsoft YaHei UI")),
            small: make(13, 400, w!("Microsoft YaHei UI")),
            heading: make(24, 700, w!("Microsoft YaHei UI")),
            value: make(18, 700, w!("Consolas")),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for font in [self.body, self.small, self.heading, self.value] {
            if !font.is_invalid() {
                unsafe {
                    let _ = DeleteObject(font.into());
                }
            }
        }
    }
}

pub unsafe fn fill(dc: HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, rect, brush);
    let _ = DeleteObject(brush.into());
}

pub unsafe fn border(dc: HDC, rect: RECT, color: COLORREF, thickness: i32) {
    for edge in [
        RECT {
            bottom: rect.top + thickness,
            ..rect
        },
        RECT {
            top: rect.bottom - thickness,
            ..rect
        },
        RECT {
            right: rect.left + thickness,
            ..rect
        },
        RECT {
            left: rect.right - thickness,
            ..rect
        },
    ] {
        fill(dc, &edge, color);
    }
}

pub fn rect(dpi: f64, x: f64, y: f64, width: f64, height: f64) -> RECT {
    RECT {
        left: (x * dpi).round() as i32,
        top: (y * dpi).round() as i32,
        right: ((x + width) * dpi).round() as i32,
        bottom: ((y + height) * dpi).round() as i32,
    }
}

pub fn contains(rect: &RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

pub fn point(lp: LPARAM) -> POINT {
    POINT {
        x: lp.0 as u16 as i16 as i32,
        y: (lp.0 >> 16) as u16 as i16 as i32,
    }
}

pub fn slider_value(rect: &RECT, x: i32, maximum: i32) -> i32 {
    let width = (rect.right - rect.left - 16).max(1);
    ((x - rect.left - 8).clamp(0, width) * maximum + width / 2) / width
}

pub fn scroll_offset(current: usize, delta: i32, count: usize, visible: usize) -> usize {
    let maximum = count.saturating_sub(visible);
    if delta < 0 {
        current.saturating_sub((-delta) as usize).min(maximum)
    } else {
        current.saturating_add(delta as usize).min(maximum)
    }
}

pub unsafe fn text(
    dc: HDC,
    rect: &mut RECT,
    value: &str,
    font: HFONT,
    color: COLORREF,
    format: DRAW_TEXT_FORMAT,
) {
    let old = SelectObject(dc, font.into());
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT);
    let mut value = wide(value);
    DrawTextW(dc, &mut value, rect, format);
    SelectObject(dc, old);
}

pub unsafe fn label(dc: HDC, rect: RECT, value: &str, font: HFONT, color: COLORREF) {
    let mut rect = rect;
    text(
        dc,
        &mut rect,
        value,
        font,
        color,
        DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
}

pub unsafe fn heart(dc: HDC, dpi: f64, x: f64, y: f64) {
    let rows = [0x36u8, 0x7f, 0x7f, 0x3e, 0x1c, 0x08];
    let pixel = (2.0 * dpi).round().max(1.0) as i32;
    for (row, bits) in rows.iter().enumerate() {
        let row = row as i32;
        for column in 0..7 {
            if bits & (1 << (6 - column)) != 0 {
                let rect = RECT {
                    left: (x * dpi).round() as i32 + column * pixel as i32,
                    top: (y * dpi).round() as i32 + row * pixel as i32,
                    right: (x * dpi).round() as i32 + (column + 1) * pixel as i32,
                    bottom: (y * dpi).round() as i32 + (row + 1) * pixel as i32,
                };
                fill(dc, &rect, RED);
            }
        }
    }
}

pub unsafe fn logo(dc: HDC, dpi: f64, x: f64, y: f64) {
    let letters = [
        [14u8, 17, 17, 31, 17, 17, 17],
        [14, 17, 17, 17, 21, 18, 13],
        [17, 17, 17, 17, 17, 17, 14],
        [14, 17, 17, 31, 17, 17, 17],
    ];
    for (letter, rows) in letters.iter().enumerate() {
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    fill(
                        dc,
                        &rect(
                            dpi,
                            x + (letter * 6 + column) as f64 * 4.0,
                            y + row as f64 * 4.0,
                            4.0,
                            4.0,
                        ),
                        WHITE,
                    );
                }
            }
        }
    }
}

pub unsafe fn button(
    dc: HDC,
    state: &State,
    dpi: f64,
    id: usize,
    rect: RECT,
    value: &str,
    font: HFONT,
    primary: bool,
) {
    let selected = state.hot == Some(id) || state.focus == Some(id);
    let color = if primary { CYAN } else { WHITE };
    if selected {
        heart(
            dc,
            dpi,
            rect.left as f64 / dpi - 18.0,
            rect.top as f64 / dpi + 11.0,
        );
    }
    label(
        dc,
        rect,
        value,
        font,
        if state.active == Some(id) {
            YELLOW
        } else {
            color
        },
    );
}

pub unsafe fn choice(
    dc: HDC,
    state: &State,
    dpi: f64,
    id: usize,
    rect: RECT,
    name: &str,
    value: &str,
    font: HFONT,
) {
    let selected = state.hot == Some(id) || state.focus == Some(id) || state.popup == Some(id);
    if selected {
        heart(
            dc,
            dpi,
            rect.left as f64 / dpi - 18.0,
            rect.top as f64 / dpi + 10.0,
        );
    }
    let name_rect = RECT {
        right: rect.left + (rect.right - rect.left) * 45 / 100,
        ..rect
    };
    label(dc, name_rect, name, font, MUTED);
    let mut value_rect = RECT {
        left: name_rect.right,
        ..rect
    };
    text(
        dc,
        &mut value_rect,
        value,
        font,
        if selected { YELLOW } else { WHITE },
        DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
    let arrow_x = rect.right - (8.0 * dpi) as i32;
    let arrow_y = rect.top + (rect.bottom - rect.top) / 2;
    for row in 0..3 {
        let width = (5 - row * 2) * dpi.round() as i32;
        fill(
            dc,
            &RECT {
                left: arrow_x - width / 2,
                top: arrow_y + row * dpi.round() as i32,
                right: arrow_x + (width + 1) / 2,
                bottom: arrow_y + (row + 1) * dpi.round() as i32,
            },
            if selected { YELLOW } else { WHITE },
        );
    }
}

pub unsafe fn slider(
    dc: HDC,
    state: &State,
    dpi: f64,
    id: usize,
    rect: RECT,
    value: i32,
    maximum: i32,
) {
    let selected = state.hot == Some(id) || state.focus == Some(id);
    let unit = dpi.round().max(1.0) as i32;
    let track = RECT {
        left: rect.left + 8 * unit,
        top: rect.top + (rect.bottom - rect.top) / 2 - unit,
        right: rect.right - 8 * unit,
        bottom: rect.top + (rect.bottom - rect.top) / 2 + unit,
    };
    fill(dc, &track, DIM);
    let width = track.right - track.left;
    let position = track.left + width * value.clamp(0, maximum) / maximum.max(1);
    fill(
        dc,
        &RECT {
            right: position,
            ..track
        },
        CYAN,
    );
    fill(
        dc,
        &RECT {
            left: position - 4 * unit,
            top: rect.top + 2 * unit,
            right: position + 4 * unit,
            bottom: rect.bottom - 2 * unit,
        },
        if selected { YELLOW } else { WHITE },
    );
}

pub fn popup_rect(anchor: &RECT, dpi: f64, count: usize) -> RECT {
    let rows = count.clamp(1, 8) as f64;
    let height = rows * 34.0 + 4.0;
    RECT {
        left: anchor.left,
        top: anchor.bottom + (4.0 * dpi).round() as i32,
        right: anchor.right,
        bottom: anchor.bottom + ((height + 4.0) * dpi).round() as i32,
    }
}

pub unsafe fn edit_color(msg: u32, wp: WPARAM) -> Option<LRESULT> {
    if msg == WM_CTLCOLOREDIT {
        let dc = HDC(wp.0 as *mut c_void);
        SetTextColor(dc, WHITE);
        SetBkColor(dc, BLACK);
        Some(LRESULT(GetStockObject(BLACK_BRUSH).0 as isize))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_slider_stays_within_endpoints() {
        let rect = RECT {
            left: 20,
            top: 0,
            right: 136,
            bottom: 30,
        };
        assert_eq!(slider_value(&rect, -100, 100), 0);
        assert_eq!(slider_value(&rect, 28, 100), 0);
        assert_eq!(slider_value(&rect, 78, 100), 50);
        assert_eq!(slider_value(&rect, 128, 100), 100);
        assert_eq!(slider_value(&rect, 500, 100), 100);
    }

    #[test]
    fn scrolling_is_clamped_in_both_directions() {
        assert_eq!(scroll_offset(0, -3, 20, 8), 0);
        assert_eq!(scroll_offset(4, 3, 20, 8), 7);
        assert_eq!(scroll_offset(11, 3, 20, 8), 12);
        assert_eq!(scroll_offset(3, -3, 2, 8), 0);
    }
}
