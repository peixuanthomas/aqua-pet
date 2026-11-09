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
#[ignore = "changes the stacking order of two real Win32 windows"]
fn pet_recovers_topmost_when_an_ordinary_window_covers_it() {
    unsafe {
        let pet = Window::new(WS_EX_LAYERED);
        let ordinary = Window::new(WINDOW_EX_STYLE(0));
        let mut app = App::new(
            Catalog::load_desktop().unwrap(),
            pet.0,
            HWND::default(),
            HWND::default(),
            true,
        );
        app.auto_exit = None;
        let work = App::work_bounds_at(POINT::default());
        app.anchor = POINT {
            x: ((work.left + work.right) / 2.0) as i32,
            y: ((work.top + work.bottom) / 2.0) as i32,
        };
        app.last_clock = Instant::now() - Duration::from_secs(3);
        app.update().unwrap();
        let _ = ShowWindow(pet.0, SW_SHOWNOACTIVATE);
        let mut rect = RECT::default();
        GetWindowRect(pet.0, &mut rect).unwrap();
        SetWindowPos(
            ordinary.0,
            Some(HWND_TOP),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
        .unwrap();
        let pet_is_above = || {
            std::iter::successors(Some(ordinary.0), |hwnd| GetWindow(*hwnd, GW_HWNDPREV).ok())
                .take(1024)
                .any(|hwnd| hwnd == pet.0)
        };
        assert!(pet_is_above());

        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        SetWindowPos(pet.0, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags).unwrap();
        SetWindowPos(ordinary.0, Some(HWND_TOP), 0, 0, 0, 0, flags).unwrap();
        assert!(!pet_is_above());

        let foreground = GetForegroundWindow();
        app.last_clock = Instant::now() - Duration::from_secs(3);
        app.update().unwrap();
        assert!(pet_is_above(), "the pet stayed behind the ordinary window");
        assert_eq!(GetForegroundWindow(), foreground);
        let mut after = RECT::default();
        GetWindowRect(pet.0, &mut after).unwrap();
        assert_eq!(after, rect);
    }
}

#[test]
fn release_velocity_tracks_recent_motion_and_expires_when_the_cursor_stops() {
    let time = u32::MAX - 30;
    let start = POINT { x: -200, y: 110 };
    let mut drag = Drag::new(start, time, POINT::default());
    drag.move_to(POINT { x: -180, y: 100 }, time.wrapping_add(20));
    assert_eq!(drag.release_velocity(time.wrapping_add(20)), (0.0, 0.0));
    let end = POINT { x: -160, y: 90 };
    drag.move_to(end, time.wrapping_add(40));
    assert_eq!(
        drag.release_velocity(time.wrapping_add(40)),
        (500.0, -250.0)
    );

    drag.move_to(end, time.wrapping_add(120));
    assert_eq!(drag.release_velocity(time.wrapping_add(120)), (0.0, 0.0));

    drag.move_to(POINT { x: -170, y: 95 }, time.wrapping_add(130));
    drag.move_to(POINT { x: -220, y: 100 }, time.wrapping_add(140));
    assert!(drag.release_velocity(time.wrapping_add(140)).0 < 0.0);
    drag.move_to(POINT { x: -215, y: 100 }, time.wrapping_add(150));
    assert_eq!(drag.release_velocity(time.wrapping_add(150)), (0.0, 0.0));
}

#[test]
#[ignore = "checks drag animation, releases and throws in a real Win32 window"]
fn dragging_uses_sleeve_frames_and_launches_from_the_release_position() {
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
        for (release, distance, paused, landing) in [
            (WM_CAPTURECHANGED, 20, false, Pose::Idle),
            (WM_LBUTTONUP, 20, false, Pose::ClapJump),
            (WM_LBUTTONUP, 80, true, Pose::ClapJump),
            (WM_LBUTTONUP, 80, false, Pose::Collapsed),
        ] {
            app.anchor = anchor;
            app.show_pose(Pose::Perch);
            app.handle(pet.0, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0));
            assert_eq!(app.auto_pose, Pose::Perch);
            let (mut start, time) = mouse_sample();
            start.x -= distance;
            app.drag = Some(Drag::new(start, time.wrapping_sub(40), anchor));
            app.handle(pet.0, WM_MOUSEMOVE, WPARAM(0), LPARAM(0));
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
            if paused {
                for (_, at) in &mut app.drag.as_mut().unwrap().samples {
                    *at = at.wrapping_sub(DRAG_MOTION_MS + 1);
                }
            }
            app.handle(pet.0, release, WPARAM(0), LPARAM(0));
            assert!(app.drag.is_none());
            assert!(app.wander.support.is_none());
            assert!((app.wander.foot.0 - expected_foot.0).abs() <= 0.5);
            assert!((app.wander.foot.1 - expected_foot.1).abs() <= 0.5);
            let released_x = app.wander.foot.0;
            let scale = app.effective_scale;
            let world = crate::ledges::Ledges::new(work, app.poses.body(scale), &[]);
            for _ in 0..120 {
                app.wander.advance(1.0 / 60.0, 30.0, scale, &world);
                if app.wander.support.is_some() {
                    break;
                }
            }
            assert_eq!(app.wander.foot.1, world.limits.bottom);
            assert_eq!(app.wander.pose(), landing);
            if landing == Pose::Collapsed {
                assert!(app.wander.foot.0 > expected_foot.0 + 5.0);
            } else {
                assert_eq!(app.wander.foot.0, released_x);
            }
        }
    }
}

#[test]
#[ignore = "checks left clicks and flattery frames in a real Win32 window"]
fn clicking_the_pet_plays_flattery_in_place() {
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
        let foot = ((work.left + work.right) / 2.0, work.bottom);
        app.wander.place(foot);
        app.update().unwrap();

        app.handle(pet.0, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0));
        assert_eq!(app.auto_pose, Pose::Idle);
        app.handle(pet.0, WM_LBUTTONUP, WPARAM(0), LPARAM(0));
        assert_eq!(app.auto_pose, Pose::Flattery);
        let clip = &app.catalog.clips[app.player.selected];
        assert_eq!(clip.id, "act-flattery");
        assert_eq!(app.player.index(clip), 0);

        app.last_clock = Instant::now() - Duration::from_millis(950);
        app.update().unwrap();
        let clip = &app.catalog.clips[app.player.selected];
        assert_eq!(app.player.index(clip), clip.steps.len() - 1);
        app.last_clock = Instant::now() - Duration::from_millis(250);
        app.update().unwrap();
        assert_eq!(app.auto_pose, Pose::Idle);
        assert_eq!(app.wander.foot, foot);
    }
}

#[test]
#[ignore = "checks seated animation alignment against a real window frame"]
fn seated_frames_keep_their_seat_on_a_real_window_edge() {
    use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let work = App::work_bounds_at(POINT::default());
        let x = work.left as i32 + 160;
        let y = work.top as i32 + 360;
        let ledge = Window(
            CreateWindowExW(
                WS_EX_TOPMOST,
                w!("STATIC"),
                w!("AQUA 坐姿对齐测试"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                x,
                y,
                600,
                240,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let pet = Window::new(WS_EX_LAYERED | WS_EX_TOPMOST);
        text(pet.0, "AQUA 边沿坐姿");
        let mut app = App::new(
            Catalog::load_desktop().unwrap(),
            pet.0,
            HWND::default(),
            HWND::default(),
            true,
        );
        app.auto_exit = None;
        let _ = ShowWindow(pet.0, SW_SHOWNOACTIVATE);
        let mut edge = RECT::default();
        DwmGetWindowAttribute(
            ledge.0,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut edge).cast(),
            size_of::<RECT>() as u32,
        )
        .unwrap();
        app.wander.foot = (edge.left as f64 + 220.0, edge.top as f64);
        for scale in 1..=8 {
            app.effective_scale = scale;
            app.sync_anchor();
            for pose in [Pose::Perch, Pose::PerchWave] {
                app.show_pose(pose);
                let count = app.catalog.clips[app.player.selected].steps.len();
                for index in 0..count {
                    let clip = &app.catalog.clips[app.player.selected];
                    app.player.seek(clip, clip.step_start(index));
                    app.last_clock = Instant::now() - Duration::from_secs(3);
                    app.update().unwrap();
                    let mut rect = RECT::default();
                    GetWindowRect(pet.0, &mut rect).unwrap();
                    let (sx, sy) = app.poses.get(pose).foot(index);
                    assert_eq!(
                        rect.top + ((sy - app.bounds.top) * scale as f64) as i32,
                        edge.top,
                        "{pose:?} frame {index} at {scale}x"
                    );
                    assert_eq!(
                        rect.left as f64 + (sx - app.bounds.left) * scale as f64,
                        app.wander.foot.0
                    );
                }
            }
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

        let walk = app.settings_regions[&WALK_CHANCE].bounds;
        let distance = app.settings_regions[&DISTANCE_BIAS].bounds;
        let dance = app.settings_regions[&DANCE_TO_MUSIC].bounds;
        app.settings_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(distance));
        check_markers(settings.0, distance, distance, true, |dc, client| {
            app.paint_settings(dc, client)
        });
        app.settings_mouse(WM_MOUSELEAVE, WPARAM(0), LPARAM(0));
        check_markers(settings.0, distance, distance, true, |dc, client| {
            app.paint_settings(dc, client)
        });

        app.settings_tab(true);
        assert_eq!(app.settings_ui.focus, Some(WALK_CHANCE));
        app.settings_tab(false);
        assert_eq!(app.settings_ui.focus, Some(DISTANCE_BIAS));
        app.settings_mouse(WM_MOUSEMOVE, WPARAM(0), mouse_point(dance));
        check_markers(settings.0, dance, dance, true, |dc, client| {
            app.paint_settings(dc, client)
        });
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(dance));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(walk));
        assert!(app.settings_draft.dance_to_music);
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(dance));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(dance));
        assert!(!app.settings_draft.dance_to_music);
        assert!(app.dance_to_music);
        check_markers(settings.0, dance, dance, true, |dc, client| {
            app.paint_settings(dc, client)
        });
        let space = WPARAM(VK_SPACE.0 as usize);
        app.handle(settings.0, WM_KEYDOWN, space, LPARAM(0));
        assert!(app.settings_draft.dance_to_music);
        app.handle(settings.0, WM_KEYDOWN, space, LPARAM(1 << 30));
        assert!(app.settings_draft.dance_to_music);

        app.settings_ui.focus = Some(WALK_CHANCE);
        let before = app.wander.probability();
        app.settings_key(VK_RIGHT);
        assert_eq!(app.settings_draft.walk_probability, (before + 5).min(100));
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(walk));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(walk));
        assert_eq!(app.settings_draft.walk_probability, (before + 10).min(100));
        assert_eq!(app.wander.probability(), before);
        app.handle(settings.0, WM_CLOSE, WPARAM(0), LPARAM(0));
        assert_eq!(app.settings_draft.walk_probability, before);

        app.settings_key(VK_RIGHT);
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(walk));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(walk));
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(dance));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(dance));
        app.settings_key(VK_DOWN);
        assert_eq!(app.settings_ui.focus, Some(DEBUG_MODE));
        app.settings_key(VK_DOWN);
        assert_eq!(app.settings_ui.focus, Some(settings::SAVE));
        app.settings_key(VK_RETURN);
        assert!(app.settings_save == settings::SaveState::Saved);
        assert_eq!(app.wander.probability(), (before + 10).min(100));
        assert!(!app.dance_to_music);
        app.handle(
            settings.0,
            WM_KEYDOWN,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(1 << 30),
        );
        assert!(app.settings_save == settings::SaveState::Saved);
        app.settings_key(VK_ESCAPE);
        assert!(app.settings_save == settings::SaveState::Editing);
    }
}

#[test]
fn only_saved_debug_mode_exposes_the_action_preview_menu() {
    unsafe {
        let settings = Window::new(WS_EX_COMPOSITED);
        let mut app = App::new(
            Catalog::load_desktop().unwrap(),
            HWND::default(),
            HWND::default(),
            settings.0,
            true,
        );
        app.create_settings_panel();
        let shows_preview = |app: &App| {
            let menu = app.context_menu().unwrap();
            let present = GetMenuState(menu, MENU_DEBUG, MF_BYCOMMAND) != u32::MAX;
            DestroyMenu(menu).unwrap();
            present
        };
        assert!(!shows_preview(&app));

        app.settings_ui.focus = Some(DEBUG_MODE);
        app.settings_key(VK_RIGHT);
        assert!(app.settings_draft.debug_mode);
        assert!(!shows_preview(&app));
        app.close_settings();
        assert!(!app.settings_draft.debug_mode);

        let row = app.settings_regions[&DEBUG_MODE].bounds;
        app.settings_mouse(WM_LBUTTONDOWN, WPARAM(0), mouse_point(row));
        app.settings_mouse(WM_LBUTTONUP, WPARAM(0), mouse_point(row));
        assert!(app.settings_draft.debug_mode);
        app.settings_key(VK_DOWN);
        app.settings_key(VK_RETURN);
        assert!(shows_preview(&app));

        app.settings_key(VK_ESCAPE);
        app.settings_ui.focus = Some(DEBUG_MODE);
        app.settings_key(VK_LEFT);
        app.settings_key(VK_DOWN);
        app.settings_key(VK_RETURN);
        assert!(!shows_preview(&app));
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
