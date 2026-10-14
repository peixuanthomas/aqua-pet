use super::*;

pub const BLACK: COLORREF = COLORREF(0x000000);
pub const WHITE: COLORREF = COLORREF(0xffffff);
pub const CYAN: COLORREF = COLORREF(0xf5e469);
pub const PURPLE: COLORREF = COLORREF(0xa63c70);
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
        WM_MEASUREITEM => {
            let item = &mut *(lp.0 as *mut MEASUREITEMSTRUCT);
            item.itemHeight = 34 * GetDpiForWindow(hwnd).max(96) / 96;
            Some(LRESULT(1))
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            let dc = HDC(wp.0 as *mut c_void);
            let id = GetDlgCtrlID(HWND(lp.0 as *mut c_void)) as usize;
            let color = match id {
                WALK_VALUE | DISTANCE_VALUE | ACTION_TITLE => CYAN,
                WALK_OFF | WALK_OFTEN | DISTANCE_SHORT | DISTANCE_LONG | SETTINGS_NOTE | COUNT
                | STATUS | EMPTY | 306 | 308 => MUTED,
                _ => WHITE,
            };
            SetTextColor(dc, color);
            SetBkColor(dc, BLACK);
            Some(LRESULT(GetStockObject(BLACK_BRUSH).0 as isize))
        }
        WM_NOTIFY => {
            let header = &*(lp.0 as *const NMHDR);
            if header.code != NM_CUSTOMDRAW
                || ![WALK_CHANCE, DISTANCE_BIAS, TIMELINE].contains(&header.idFrom)
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
                        // Windows only clears this rectangle when the thumb moves.
                        fill(draw.hdc, &draw.rc, CYAN);
                        border(draw.hdc, draw.rc, BLACK, dpi.round() as i32);
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
        WM_DRAWITEM => {
            let draw = &*(lp.0 as *const DRAWITEMSTRUCT);
            let selected = draw.itemState.0 & ODS_SELECTED.0 != 0;
            let focused = draw.itemState.0 & ODS_FOCUS.0 != 0;
            let button = draw.CtlType == ODT_BUTTON;
            let primary = draw.CtlID as usize == PLAY;
            let dpi = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
            let background = if selected {
                PURPLE
            } else if primary {
                CYAN
            } else {
                BLACK
            };
            fill(draw.hDC, &draw.rcItem, background);
            SetTextColor(draw.hDC, if primary && !selected { BLACK } else { WHITE });
            SetBkMode(draw.hDC, TRANSPARENT);
            let font = send(draw.hwndItem, WM_GETFONT, 0, 0);
            let old = SelectObject(draw.hDC, HGDIOBJ(font as *mut c_void));
            let mut label: Vec<u16> = if button {
                window_text(draw.hwndItem).encode_utf16().collect()
            } else if draw.itemID != u32::MAX {
                let (length, get) = if draw.CtlType == ODT_LISTBOX {
                    (LB_GETTEXTLEN, LB_GETTEXT)
                } else {
                    (CB_GETLBTEXTLEN, CB_GETLBTEXT)
                };
                let len = send(draw.hwndItem, length, draw.itemID as usize, 0).max(0) as usize;
                let mut buffer = vec![0u16; len + 1];
                send(
                    draw.hwndItem,
                    get,
                    draw.itemID as usize,
                    buffer.as_mut_ptr() as isize,
                );
                buffer.truncate(len);
                buffer
            } else {
                Vec::new()
            };
            let mut area = draw.rcItem;
            area.left += (12.0 * dpi).round() as i32;
            area.right -= (8.0 * dpi).round() as i32;
            DrawTextW(
                draw.hDC,
                &mut label,
                &mut area,
                DT_SINGLELINE
                    | DT_VCENTER
                    | DT_NOPREFIX
                    | DT_END_ELLIPSIS
                    | if button { DT_CENTER } else { DT_LEFT },
            );
            SelectObject(draw.hDC, old);
            if button || focused {
                border(
                    draw.hDC,
                    draw.rcItem,
                    if focused { CYAN } else { WHITE },
                    (2.0 * dpi).round() as i32,
                );
            }
            if focused {
                let inset = (4.0 * dpi).round() as i32;
                let focus = RECT {
                    left: draw.rcItem.left + inset,
                    top: draw.rcItem.top + inset,
                    right: draw.rcItem.right - inset,
                    bottom: draw.rcItem.bottom - inset,
                };
                border(
                    draw.hDC,
                    focus,
                    if primary && !selected { BLACK } else { CYAN },
                    dpi.round() as i32,
                );
            }
            Some(LRESULT(1))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_thumb_leaves_no_pixels_outside_its_native_bounds() {
        unsafe {
            let surface = Surface::new(64, 32).unwrap();
            let client = RECT {
                left: 0,
                top: 0,
                right: 64,
                bottom: 32,
            };
            fill(surface.dc, &client, BLACK);
            let mut draw = NMCUSTOMDRAW {
                hdr: NMHDR {
                    idFrom: WALK_CHANCE,
                    code: NM_CUSTOMDRAW,
                    ..Default::default()
                },
                dwDrawStage: CDDS_ITEMPREPAINT,
                hdc: surface.dc,
                rc: RECT {
                    left: 10,
                    top: 4,
                    right: 19,
                    bottom: 28,
                },
                dwItemSpec: TBCD_THUMB as usize,
                ..Default::default()
            };
            control_message(
                HWND::default(),
                WM_NOTIFY,
                WPARAM(0),
                LPARAM((&draw as *const NMCUSTOMDRAW) as isize),
            );
            // Windows only clears the old native thumb rectangle when it moves.
            fill(surface.dc, &draw.rc, BLACK);
            draw.rc.left = 35;
            draw.rc.right = 44;
            control_message(
                HWND::default(),
                WM_NOTIFY,
                WPARAM(0),
                LPARAM((&draw as *const NMCUSTOMDRAW) as isize),
            );
            assert_eq!(GetPixel(surface.dc, 39, 16), CYAN);
            for y in 0..32 {
                for x in 0..64 {
                    if x < draw.rc.left
                        || x >= draw.rc.right
                        || y < draw.rc.top
                        || y >= draw.rc.bottom
                    {
                        assert_eq!(
                            GetPixel(surface.dc, x, y),
                            BLACK,
                            "stale pixel at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}
