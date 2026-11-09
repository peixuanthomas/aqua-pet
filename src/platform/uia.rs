use super::*;
use std::sync::mpsc::{self, Receiver, Sender};
use windows::Win32::{
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    },
    UI::{
        Accessibility::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
    },
};

const QUERY_INTERVAL: Duration = Duration::from_millis(50);
const TRANSPARENT: COLORREF = COLORREF(0x030201);

pub(super) struct Inspector {
    outline: HWND,
    shown: Option<(HWND, RECT)>,
    pending: Option<HWND>,
    next_query: Instant,
    requests: Sender<POINT>,
    results: Receiver<Option<RECT>>,
}

impl Inspector {
    pub unsafe fn new(owner: HWND) -> windows::core::Result<Self> {
        let outline = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            None,
            WS_POPUP | WS_DISABLED,
            0,
            0,
            0,
            0,
            Some(owner),
            None,
            None,
            None,
        )?;
        let (requests, read_requests) = mpsc::channel();
        let (write_results, results) = mpsc::channel();
        let inspector = Self {
            outline,
            shown: None,
            pending: None,
            next_query: Instant::now(),
            requests,
            results,
        };
        SetLayeredWindowAttributes(outline, TRANSPARENT, 0, LWA_COLORKEY)?;
        SetWindowSubclass(outline, Some(paint_outline), 1, 0).ok()?;
        std::thread::spawn(move || unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() {
                let _ = read_controls(read_requests, write_results);
                CoUninitialize();
            }
        });
        Ok(inspector)
    }

    pub unsafe fn hide(&mut self) {
        if self.shown.take().is_some() {
            let _ = ShowWindow(self.outline, SW_HIDE);
        }
    }

    pub unsafe fn update(&mut self, now: Instant) {
        let mut point = POINT::default();
        let window = if GetPhysicalCursorPos(&mut point).is_ok() {
            WindowFromPoint(point)
        } else {
            HWND::default()
        };
        if self
            .shown
            .is_some_and(|(target, rect)| target != window || !ui::contains(&rect, point))
        {
            self.hide();
        }
        if let Ok(result) = self.results.try_recv()
            && self.pending.take() == Some(window)
        {
            let rect = result.filter(|rect| ui::contains(rect, point));
            if let Some(rect) = rect {
                self.show(window, rect);
            } else {
                self.hide();
            }
        }
        if self.pending.is_none() && !window.is_invalid() && now >= self.next_query {
            self.next_query = now + QUERY_INTERVAL;
            let mut pid = 0;
            GetWindowThreadProcessId(window, Some(&mut pid));
            if pid != std::process::id() && self.requests.send(point).is_ok() {
                self.pending = Some(window);
            }
        }
    }

    unsafe fn show(&mut self, target: HWND, rect: RECT) {
        if self.shown == Some((target, rect)) {
            return;
        }
        if SetWindowPos(
            self.outline,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.right.saturating_sub(rect.left),
            rect.bottom.saturating_sub(rect.top),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        )
        .is_ok()
        {
            self.shown = Some((target, rect));
            let _ = InvalidateRect(Some(self.outline), None, false);
        } else {
            self.hide();
        }
    }
}

impl Drop for Inspector {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.outline);
        }
    }
}

unsafe fn read_controls(
    requests: Receiver<POINT>,
    results: Sender<Option<RECT>>,
) -> windows::core::Result<()> {
    let automation: IUIAutomation2 = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
    automation.SetConnectionTimeout(250)?;
    automation.SetTransactionTimeout(250)?;
    let cache = automation.CreateCacheRequest()?;
    for property in [
        UIA_IsControlElementPropertyId,
        UIA_IsOffscreenPropertyId,
        UIA_ControlTypePropertyId,
        UIA_BoundingRectanglePropertyId,
    ] {
        cache.AddProperty(property)?;
    }
    let walker = automation.ControlViewWalker()?;
    while let Ok(point) = requests.recv() {
        let rect = (|| {
            let hit = automation.ElementFromPointBuildCache(point, &cache).ok()?;
            let control = if hit.CachedIsControlElement().ok()?.as_bool() {
                hit
            } else {
                walker.NormalizeElementBuildCache(&hit, &cache).ok()?
            };
            let kind = control.CachedControlType().ok()?;
            if control.CachedIsOffscreen().ok()?.as_bool()
                || kind == UIA_WindowControlTypeId
                || kind == UIA_PaneControlTypeId
            {
                return None;
            }
            control
                .CachedBoundingRectangle()
                .ok()
                .filter(|rect| ui::contains(rect, point))
        })();
        if results.send(rect).is_err() {
            break;
        }
    }
    Ok(())
}

unsafe extern "system" fn paint_outline(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message != WM_PAINT {
        return DefSubclassProc(hwnd, message, wp, lp);
    }
    let mut paint = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    ui::fill(dc, &paint.rcPaint, TRANSPARENT);
    if rect.right > 6 && rect.bottom > 6 {
        ui::border(dc, rect, ui::BLACK, 3);
        rect.left += 1;
        rect.top += 1;
        rect.right -= 1;
        rect.bottom -= 1;
        ui::border(dc, rect, ui::YELLOW, 2);
    } else {
        ui::fill(dc, &rect, ui::YELLOW);
    }
    let _ = EndPaint(hwnd, &paint);
    LRESULT(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads and outlines a real Win32 button on the interactive desktop"]
    fn reads_standard_button_without_blocking_mouse_hits() {
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }

        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let button = Window(
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("BUTTON"),
                    w!("AQUA UIA test"),
                    WS_POPUP,
                    80,
                    80,
                    160,
                    48,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap(),
            );
            let _ = ShowWindow(button.0, SW_SHOWNOACTIVATE);
            let mut rect = RECT::default();
            GetWindowRect(button.0, &mut rect).unwrap();
            let point = POINT {
                x: rect.left + 1,
                y: rect.top + 16,
            };
            assert_eq!(WindowFromPoint(point), button.0);

            let mut inspector = Inspector::new(button.0).unwrap();
            inspector.requests.send(point).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let result = loop {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&msg);
                }
                if let Ok(result) = inspector.results.try_recv() {
                    break result;
                }
                assert!(Instant::now() < deadline, "UI Automation did not return");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(result, Some(rect));

            let foreground = GetForegroundWindow();
            inspector.show(button.0, rect);
            assert!(IsWindowVisible(inspector.outline).as_bool());
            assert_eq!(WindowFromPoint(point), button.0);
            assert_eq!(GetForegroundWindow(), foreground);
            inspector.hide();
            assert!(!IsWindowVisible(inspector.outline).as_bool());
        }
    }
}
