use super::*;

struct Window(HWND);

impl Window {
    unsafe fn new(style: WINDOW_EX_STYLE) -> Self {
        static REGISTER: std::sync::Once = std::sync::Once::new();
        let module = GetModuleHandleW(None).unwrap();
        let class = w!("AquaPetPanelTest");
        REGISTER.call_once(|| {
            assert_ne!(
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(wndproc),
                    hInstance: module.into(),
                    lpszClassName: class,
                    ..Default::default()
                }),
                0
            );
        });
        Self(
            CreateWindowExW(
                style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                None,
                WS_POPUP,
                -20000,
                -20000,
                960,
                720,
                None,
                None,
                Some(module.into()),
                None,
            )
            .unwrap(),
        )
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

fn mouse_point(rect: RECT) -> LPARAM {
    let x = (rect.left + rect.right) / 2;
    let y = (rect.top + rect.bottom) / 2;
    LPARAM(((y << 16) | (x & 0xffff)) as isize)
}

#[test]
#[ignore = "checks drag animation and release positions in a real Win32 window"]
fn dragging_uses_sleeve_frames_and_releases_in_place() {
    unsafe {
        let pet = Window::new(WS_EX_LAYERED);
        let mut app = App::new(
            Catalog::load_desktop().unwrap(),
            pet.0,
            HWND::default(),
            HWND::default(),
            true,
        );
        app.auto_exit = None;
        let work = App::work_bounds_at(POINT::default());
        let anchor = POINT {
            x: ((work.left + work.right) / 2.0) as i32,
            y: ((work.top + work.bottom) / 2.0) as i32,
        };
        for release in [WM_LBUTTONUP, WM_CAPTURECHANGED] {
            app.anchor = anchor;
            app.handle(pet.0, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0));
            assert_eq!(app.auto_pose, Pose::Sleeve);
            assert_eq!(app.catalog.clips[app.player.selected].id, "sleeve-garden");
            let mut expected_foot = (0.0, 0.0);
            for scale in 1..=8 {
                app.scale = scale;
                app.effective_scale = scale;
                let count = app.catalog.clips[app.player.selected].steps.len();
                for index in 0..count {
                    let clip = &app.catalog.clips[app.player.selected];
                    app.player.seek(clip, clip.step_start(index));
                    app.update().unwrap();
                    let mut rect = RECT::default();
                    GetWindowRect(pet.0, &mut rect).unwrap();
                    let (fx, fy) = app.poses.get(Pose::Sleeve).foot(index);
                    expected_foot = (
                        rect.left as f64 + (fx - app.bounds.left) * scale as f64,
                        rect.top as f64 + (fy - app.bounds.top) * scale as f64,
                    );
                }
            }
            app.handle(pet.0, release, WPARAM(0), LPARAM(0));
            assert!(app.drag.is_none());
            assert!(app.wander.support.is_none());
            assert!((app.wander.foot.0 - expected_foot.0).abs() <= 0.5);
            assert!((app.wander.foot.1 - expected_foot.1).abs() <= 0.5);
        }
    }
}

unsafe fn check_markers(
    hwnd: HWND,
    heart_row: RECT,
    hover_row: RECT,
    yellow: bool,
    draw: impl FnOnce(HDC, RECT),
) {
    let mut client = RECT::default();
    GetClientRect(hwnd, &mut client).unwrap();
    let surface = Surface::new(client.right, client.bottom).unwrap();
    ui::fill(surface.dc, &client, ui::BLACK);
    draw(surface.dc, client);
    assert!(GdiFlush().as_bool());
    let pixels = std::slice::from_raw_parts(
        surface.bits.cast::<u32>(),
        (client.right * client.bottom) as usize,
    );
    let mut red_count = 0;
    let mut has_yellow = false;
    for (y, row) in pixels.chunks_exact(client.right as usize).enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            let point = POINT {
                x: x as i32,
                y: y as i32,
            };
            match pixel & 0x00ffffff {
                0xff0000 => {
                    assert!(
                        ui::contains(&heart_row, point),
                        "red outside selected row at {point:?}"
                    );
                    red_count += 1;
                }
                0xffff00 if ui::contains(&hover_row, point) => has_yellow = true,
                _ => {}
            }
        }
    }
    let pixel_size = (2.0 * GetDpiForWindow(hwnd).max(96) as f64 / 96.0).round() as i32;
    assert_eq!(red_count, 27 * pixel_size * pixel_size);
    assert_eq!(has_yellow, yellow);
}

#[test]
#[ignore = "checks hover, selection pixels and real Win32 update regions"]
fn hovering_keeps_one_selected_heart_and_only_repaints_changed_rows() {
    unsafe {
        let panel = Window::new(WS_EX_COMPOSITED);
        let settings = Window::new(WS_EX_COMPOSITED);
        let _ = ShowWindow(panel.0, SW_SHOWNOACTIVATE);
        let mut app = App::new(
            Catalog::load().unwrap(),
            HWND::default(),
            panel.0,
            settings.0,
            true,
        );
        app.create_debug_controls().unwrap();
        app.create_settings_panel();
        app.debug_open = true;
        app.select(app.visible[0]);
        let dpi = GetDpiForWindow(panel.0).max(96) as f64 / 96.0;
        let row_height = (36.0 * dpi).round() as i32;
        let list = app.debug_regions[&LIST];
        let first = RECT {
            bottom: list.top + row_height,
            ..list
        };
        let second = RECT {
            top: first.bottom,
            bottom: first.bottom + row_height,
            ..list
        };
        let _ = ValidateRect(Some(panel.0), None);
        app.debug_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(second));
        let mut area = RECT::default();
        assert!(GetUpdateRect(panel.0, Some(&mut area), false).as_bool());
        assert_eq!(area, second);
        check_markers(panel.0, first, second, true, |dc, client| {
            app.paint(dc, client)
        });
        let _ = ValidateRect(Some(panel.0), None);
        app.debug_mouse(WM_MOUSEMOVE, WPARAM(0), LPARAM(mouse_point(second).0 + 1));
        assert!(!GetUpdateRect(panel.0, None, false).as_bool());
        app.debug_mouse(WM_MOUSELEAVE, WPARAM(0), LPARAM(0));
        check_markers(panel.0, first, second, false, |dc, client| {
            app.paint(dc, client)
        });

        let kind = app.debug_regions[&KIND];
        for msg in [WM_LBUTTONDOWN, WM_LBUTTONUP] {
            app.debug_mouse(msg, WPARAM(0), mouse_point(kind));
        }
        let popup = ui::popup_rect(&kind, dpi, 4);
        let height = (34.0 * dpi).round() as i32;
        let top = popup.top + (2.0 * dpi) as i32;
        let current = RECT {
            top,
            bottom: top + height,
            ..popup
        };
        let other = RECT {
            top: current.bottom,
            bottom: current.bottom + height,
            ..popup
        };
        app.debug_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(other));
        check_markers(panel.0, current, other, true, |dc, client| {
            app.paint(dc, client)
        });
        assert_eq!(app.kind_filter, 0);
        app.debug_key(VK_DOWN);
        assert_eq!(app.debug_ui.popup, Some(KIND));
        assert_eq!(app.kind_filter, 1);
        check_markers(panel.0, other, current, false, |dc, client| {
            app.paint(dc, client)
        });
        app.debug_key(VK_ESCAPE);
        let play = app.debug_regions[&PLAY];
        app.debug_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(play));
        let anchor = RECT {
            left: kind.left - (20.0 * dpi).round() as i32,
            ..kind
        };
        check_markers(panel.0, anchor, play, true, |dc, client| {
            app.paint(dc, client)
        });

        let mut client = RECT::default();
        GetClientRect(settings.0, &mut client).unwrap();
        let dpi = GetDpiForWindow(settings.0).max(96) as f64 / 96.0;
        let height = ((client.bottom as f64 / dpi - 144.0) / 2.0).max(112.0);
        let width = client.right as f64 / dpi - 48.0;
        let walk = ui::rect(dpi, 24.0, 88.0, width, height);
        let distance = ui::rect(dpi, 24.0, 88.0 + height, width, height);
        app.settings_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(distance));
        check_markers(settings.0, walk, distance, true, |dc, client| {
            app.paint_settings(dc, client)
        });
        app.settings_mouse(WM_MOUSELEAVE, WPARAM(0), LPARAM(0));
        check_markers(settings.0, walk, distance, false, |dc, client| {
            app.paint_settings(dc, client)
        });
    }
}

#[test]
#[ignore = "checks real Win32 update regions on an interactive desktop"]
fn playback_only_repaints_progress_while_advancing() {
    unsafe {
        let pet = Window::new(WS_EX_LAYERED);
        let panel = Window::new(WS_EX_COMPOSITED);
        let _ = ShowWindow(panel.0, SW_SHOWNOACTIVATE);
        let mut app = App::new(
            Catalog::load().unwrap(),
            pet.0,
            panel.0,
            HWND::default(),
            true,
        );
        // Windows can request a paint before the user opens the Debug panel.
        assert!(
            app.handle(panel.0, WM_PAINT, WPARAM(0), LPARAM(0))
                .is_none()
        );
        app.debug_open = true;
        app.layout();
        app.player.playing = false;
        app.update().unwrap();
        let _ = ValidateRect(Some(panel.0), None);
        app.last_status = Instant::now() - Duration::from_secs(1);
        app.update().unwrap();
        let mut area = RECT::default();
        assert!(!GetUpdateRect(panel.0, Some(&mut area), false).as_bool());

        app.player.playing = true;
        app.last_clock = Instant::now();
        app.update().unwrap();
        assert!(GetUpdateRect(panel.0, Some(&mut area), false).as_bool());
        assert_eq!(
            area,
            RECT {
                bottom: app.debug_regions[&PLAY].bottom,
                ..app.debug_regions[&TIMELINE]
            }
        );
    }
}
