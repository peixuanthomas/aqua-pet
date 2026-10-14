use super::*;

pub const BLACK: COLORREF = COLORREF(0x000000);
pub const WHITE: COLORREF = COLORREF(0xffffff);
pub const CYAN: COLORREF = COLORREF(0xf5e469);
pub const MUTED: COLORREF = COLORREF(0xbdb6b6);

#[derive(Default)]
pub struct Fonts {
    pub body: HFONT,
    pub heading: HFONT,
    pub value: HFONT,
}

impl Fonts {
    pub unsafe fn new(window: HWND) -> Self {
        let dpi = GetDpiForWindow(window).max(96) as i32;
        let make = |size: i32, weight| {
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
                w!("Microsoft YaHei UI"),
            )
        };
        Self {
            body: make(16, 400),
            heading: make(24, 700),
            value: make(20, 700),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for font in [self.body, self.heading, self.value] {
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

pub unsafe fn control_message(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            let dc = HDC(wp.0 as *mut c_void);
            let id = GetDlgCtrlID(HWND(lp.0 as *mut c_void)) as usize;
            let color = match id {
                WALK_VALUE | DISTANCE_VALUE => CYAN,
                WALK_OFF | WALK_OFTEN | DISTANCE_SHORT | DISTANCE_LONG | SETTINGS_NOTE => MUTED,
                _ => WHITE,
            };
            SetTextColor(dc, color);
            SetBkColor(dc, BLACK);
            Some(LRESULT(GetStockObject(BLACK_BRUSH).0 as isize))
        }
        WM_NOTIFY => {
            let header = &*(lp.0 as *const NMHDR);
            if header.code != NM_CUSTOMDRAW
                || ![WALK_CHANCE, DISTANCE_BIAS].contains(&header.idFrom)
            {
                return None;
            }
            let draw = &*(lp.0 as *const NMCUSTOMDRAW);
            let dpi = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
            match draw.dwDrawStage {
                CDDS_PREPAINT => {
                    fill(draw.hdc, &draw.rc, BLACK);
                    Some(LRESULT(
                        (CDRF_NOTIFYITEMDRAW | CDRF_NOTIFYPOSTPAINT) as isize,
                    ))
                }
                CDDS_ITEMPREPAINT => {
                    if draw.dwItemSpec == TBCD_CHANNEL as usize {
                        fill(draw.hdc, &draw.rc, WHITE);
                    } else if draw.dwItemSpec == TBCD_THUMB as usize {
                        let center = (draw.rc.left + draw.rc.right) / 2;
                        let thumb = RECT {
                            left: center - (6.0 * dpi).round() as i32,
                            right: center + (6.0 * dpi).round() as i32,
                            ..draw.rc
                        };
                        fill(draw.hdc, &thumb, CYAN);
                        border(draw.hdc, thumb, BLACK, dpi.round() as i32);
                    }
                    Some(LRESULT(CDRF_SKIPDEFAULT as isize))
                }
                CDDS_POSTPAINT => {
                    if GetFocus() == header.hwndFrom {
                        let mut client = RECT::default();
                        let _ = GetClientRect(header.hwndFrom, &mut client);
                        border(draw.hdc, client, CYAN, dpi.round() as i32);
                    }
                    Some(LRESULT(CDRF_DODEFAULT as isize))
                }
                _ => None,
            }
        }
        _ => None,
    }
}
