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
