use super::*;
use crate::{
    ledges::{Ledges, Window},
    wander::Footing,
};
use std::{
    cell::Cell,
    sync::mpsc::{self, Receiver, Sender},
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};

const SCAN_INTERVAL: Duration = Duration::from_millis(200);
thread_local! { static WINDOWS_CHANGED: Cell<bool> = const { Cell::new(false) }; }

unsafe extern "system" fn window_changed(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if !WINDOWS_CHANGED.get()
        && child == CHILDID_SELF as i32
        && (object == OBJID_WINDOW.0 || (event == EVENT_OBJECT_REORDER && object == OBJID_CLIENT.0))
        && (event == EVENT_OBJECT_DESTROY || GetAncestor(hwnd, GA_ROOT) == hwnd)
    {
        WINDOWS_CHANGED.set(true);
    }
}

fn idle_for_two_minutes(now: u32, last_input: u32) -> bool {
    now.wrapping_sub(last_input) >= 120_000
}

pub(super) unsafe fn activity(foot: (f64, f64), body: Bounds) -> (bool, bool) {
    let mut point = POINT::default();
    let near = GetCursorPos(&mut point).is_ok() && {
        let (x, y) = (point.x as f64 - foot.0, point.y as f64 - foot.1);
        (x - x.clamp(body.left, body.right)).hypot(y - y.clamp(body.top, body.bottom))
            <= body.width()
    };
    let mut last = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        ..Default::default()
    };
    let idle =
        GetLastInputInfo(&mut last).as_bool() && idle_for_two_minutes(GetTickCount(), last.dwTime);
    (near, idle)
}

unsafe fn visible_frame(hwnd: HWND) -> Option<Bounds> {
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return None;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        (&raw mut cloaked).cast(),
        size_of::<u32>() as u32,
    );
    if cloaked != 0 {
        return None;
    }
    let mut rect = RECT::default();
    DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        (&raw mut rect).cast(),
        size_of::<RECT>() as u32,
    )
    .ok()?;
    (rect.left < rect.right && rect.top < rect.bottom).then_some(Bounds {
        left: rect.left as f64,
        right: rect.right as f64,
        top: rect.top as f64,
        bottom: rect.bottom as f64,
    })
}

unsafe fn window_info(hwnd: HWND) -> Option<Window> {
    let rect = visible_frame(hwnd)?;
    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if style & (WS_EX_LAYERED | WS_EX_TRANSPARENT).0 == (WS_EX_LAYERED | WS_EX_TRANSPARENT).0 {
        return None;
    }
    let mut name = [0u16; 128];
    let length = GetClassNameW(hwnd, &mut name) as usize;
    let class = String::from_utf16_lossy(&name[..length]);
    if matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    ) {
        return None;
    }
    let can_stand = style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).0 == 0
        && !matches!(class.as_str(), "#32768" | "tooltips_class32");
    Some(Window {
        id: hwnd.0 as usize,
        rect,
        can_stand,
    })
}

unsafe fn read_windows(own: &[usize]) -> windows::core::Result<Vec<Window>> {
    unsafe extern "system" fn collect(hwnd: HWND, param: LPARAM) -> windows::core::BOOL {
        let (own, windows) = &mut *(param.0 as *mut (&[usize], Vec<Window>));
        if !own.contains(&(hwnd.0 as usize))
            && let Some(window) = window_info(hwnd)
        {
            windows.push(window);
        }
        true.into()
    }
    let mut scan = (own, Vec::new());
    EnumWindows(Some(collect), LPARAM((&raw mut scan) as isize))?;
    Ok(scan.1)
}

pub(super) struct Desktop {
    windows: Vec<Window>,
    hooks: [HWINEVENTHOOK; 5],
    rescan_interval: Duration,
    last_scan: Instant,
    request: Sender<()>,
    snapshots: Receiver<windows::core::Result<Vec<Window>>>,
    scanning: bool,
    cached: Option<(Bounds, Bounds, Ledges)>,
}

impl Desktop {
    pub fn new(own: &[HWND]) -> Self {
        let own: Vec<_> = own.iter().map(|hwnd| hwnd.0 as usize).collect();
        let hooks = [
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_DESTROY, EVENT_OBJECT_REORDER),
            (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
        ]
        .map(|(first, last)| unsafe {
            SetWinEventHook(
                first,
                last,
                None,
                Some(window_changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        });
        // Some desktops restrict hooks. Keep polling there, and occasionally reconcile missed events elsewhere.
        let rescan_interval = if hooks.iter().any(|hook| hook.is_invalid()) {
            SCAN_INTERVAL
        } else {
            Duration::from_secs(5)
        };
        let windows = unsafe { read_windows(&own) }.unwrap_or_default();
        let (request, requests) = mpsc::channel();
        let (results, snapshots) = mpsc::channel();
        std::thread::spawn(move || {
            while requests.recv().is_ok() {
                if results.send(unsafe { read_windows(&own) }).is_err() {
                    break;
                }
            }
        });
        Self {
            windows,
            hooks,
            rescan_interval,
            last_scan: Instant::now(),
            request,
            snapshots,
            scanning: false,
            cached: None,
        }
    }

    pub unsafe fn refresh(&mut self, now: Instant, riding: Option<Footing>) -> Option<(f64, f64)> {
        let snapshot = self.snapshots.try_recv();
        let received = snapshot.is_ok();
        if let Ok(result) = snapshot {
            self.scanning = false;
            if let Ok(windows) = result
                && self.windows != windows
            {
                self.windows = windows;
                self.cached = None;
            }
        }
        let changed = WINDOWS_CHANGED.get();
        let elapsed = now.duration_since(self.last_scan);
        if !self.scanning
            && elapsed >= SCAN_INTERVAL
            && (changed || elapsed >= self.rescan_interval)
        {
            WINDOWS_CHANGED.set(false);
            self.scanning = self.request.send(()).is_ok();
            self.last_scan = now;
        }
        let s = riding?;
        let i = self.windows.iter().position(|w| w.id == s.window)?;
        // A window can move again between the background read and this animation frame.
        if changed || self.scanning || received || self.rescan_interval == SCAN_INTERVAL {
            match visible_frame(HWND(s.window as *mut c_void)) {
                Some(rect) if self.windows[i].rect == rect => {}
                Some(rect) => {
                    self.windows[i].rect = rect;
                    self.cached = None;
                }
                None => {
                    self.windows.remove(i);
                    self.cached = None;
                    return None;
                }
            }
        }
        let rect = self.windows[i].rect;
        Some((rect.left + s.offset, rect.top))
    }

    pub fn ledges(&mut self, work: Bounds, body: Bounds) -> &Ledges {
        if self
            .cached
            .as_ref()
            .is_none_or(|(w, b, _)| *w != work || *b != body)
        {
            self.cached = Some((work, body, Ledges::new(work, body, &self.windows)));
        }
        &self.cached.as_ref().unwrap().2
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        for hook in self.hooks {
            if !hook.is_invalid() {
                unsafe {
                    let _ = UnhookWinEvent(hook);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wander::Wander;
    use windows::Win32::Graphics::Dwm::DwmFlush;

    struct Fixture(HWND);

    impl Fixture {
        unsafe fn new(title: PCWSTR, x: i32, y: i32, w: i32, h: i32) -> Self {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST,
                w!("STATIC"),
                title,
                WS_OVERLAPPEDWINDOW,
                x,
                y,
                w,
                h,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE).unwrap();
            Self(hwnd)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }

    #[test]
    #[ignore = "opens two temporary windows on an interactive Windows desktop"]
    fn live_overlapping_windows_carry_the_pet_and_release_it_on_minimize() {
        unsafe extern "system" fn monitor_center(
            _: HMONITOR,
            _: HDC,
            rect: *mut RECT,
            data: LPARAM,
        ) -> windows::core::BOOL {
            let rect = *rect;
            let points = &mut *(data.0 as *mut Vec<POINT>);
            points.push(POINT {
                x: (rect.left + rect.right) / 2,
                y: (rect.top + rect.bottom) / 2,
            });
            true.into()
        }
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let mut centers = Vec::<POINT>::new();
            assert!(
                EnumDisplayMonitors(
                    None,
                    None,
                    Some(monitor_center),
                    LPARAM((&raw mut centers) as isize)
                )
                .as_bool()
            );
            assert!(!centers.is_empty());
            for center in centers {
                check_windows(App::work_bounds_at(center));
            }
        }
    }

    unsafe fn check_windows(work: Bounds) {
        let x = work.left as i32 + 40;
        let y = work.top as i32 + 240;
        let back = Fixture::new(w!("AQUA 测试 · 后窗"), x, y, 580, 200);
        let front = Fixture::new(w!("AQUA 测试 · 前窗"), x + 240, y - 70, 220, 180);
        DwmFlush().unwrap();
        let back_rect = visible_frame(back.0).unwrap();
        let front_rect = visible_frame(front.0).unwrap();
        eprintln!(
            "Checked work area {work:?} at {} DPI",
            GetDpiForWindow(back.0)
        );
        let mut desktop = Desktop::new(&[]);
        let now = Instant::now();
        desktop.refresh(now, None);
        let body = Bounds {
            left: -18.0,
            top: -36.0,
            right: 18.0,
            bottom: 0.0,
        };
        let world = desktop.ledges(work, body);
        let id = back.0.0 as usize;
        let back_edges: Vec<_> = world.edges.iter().filter(|e| e.window == id).collect();
        assert_eq!(back_edges.len(), 2);
        assert!(back_edges[0].right < front_rect.left && back_edges[1].left > front_rect.right);
        // The user's other open apps should not decide which of the two test windows wins.
        let fixture_world = Ledges {
            limits: world.limits,
            edges: world
                .edges
                .iter()
                .copied()
                .filter(|e| [crate::ledges::FLOOR, id, front.0.0 as usize].contains(&e.window))
                .collect(),
        };
        let mut roaming = Wander::for_test(42);
        roaming.set_probability(100);
        roaming.place((world.limits.right, world.limits.bottom));
        let mut jumped = false;
        let mut paced = false;
        for _ in 0..60 * 300 {
            roaming.advance(1.0 / 60.0, 30.0, 1, &fixture_world);
            jumped |= roaming.pose() == Pose::Rise;
            paced |=
                roaming.riding().is_some() && matches!(roaming.pose(), Pose::Left | Pose::Right);
            if jumped && paced {
                break;
            }
        }
        assert!(jumped && paced);
        let foot = (back_rect.left + 60.0, back_rect.top);
        let mut pet = Wander::for_test(42);
        pet.set_probability(0);
        pet.place((foot.0, foot.1 - 50.0));
        pet.advance(0.7, 30.0, 1, world);
        assert_eq!(pet.foot, foot);
        assert_eq!(pet.riding().unwrap().window, id);
        SetWindowPos(
            back.0,
            None,
            x + 30,
            y + 20,
            580,
            200,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .unwrap();
        DwmFlush().unwrap();
        // Production hooks skip windows belonging to the pet's process, including these fixtures.
        window_changed(
            HWINEVENTHOOK::default(),
            EVENT_OBJECT_LOCATIONCHANGE,
            back.0,
            OBJID_WINDOW.0,
            0,
            0,
            0,
        );
        desktop.refresh(now + Duration::from_millis(16), pet.riding());
        pet.advance(0.016, 30.0, 1, desktop.ledges(work, body));
        assert_eq!(pet.foot, (foot.0 + 30.0, foot.1 + 20.0));
        let _ = ShowWindow(back.0, SW_SHOWMINNOACTIVE);
        DwmFlush().unwrap();
        desktop.refresh(now + Duration::from_millis(32), pet.riding());
        pet.advance(0.1, 30.0, 1, desktop.ledges(work, body));
        assert!(pet.foot.1 > foot.1 + 20.0);
        assert!(pet.riding().is_none_or(|s| s.window != id));
    }

    #[test]
    fn idle_threshold_survives_the_windows_tick_counter_wrapping() {
        let last = u32::MAX - 10;
        assert!(!idle_for_two_minutes(last.wrapping_add(119_999), last));
        assert!(idle_for_two_minutes(last.wrapping_add(120_000), last));
    }

    #[test]
    fn unchanged_desktop_waits_and_changes_during_a_scan_are_coalesced() {
        let now = Instant::now();
        let (request, requests) = mpsc::channel();
        let (results, snapshots) = mpsc::channel();
        let mut desktop = Desktop {
            windows: Vec::new(),
            hooks: [HWINEVENTHOOK::default(); 5],
            rescan_interval: Duration::from_secs(5),
            last_scan: now,
            request,
            snapshots,
            scanning: false,
            cached: None,
        };
        let refresh = |desktop: &mut Desktop, ms| unsafe {
            desktop.refresh(now + Duration::from_millis(ms), None);
        };
        WINDOWS_CHANGED.set(false);
        refresh(&mut desktop, 400);
        assert!(requests.try_recv().is_err());
        WINDOWS_CHANGED.set(true);
        refresh(&mut desktop, 500);
        requests.try_recv().unwrap();
        WINDOWS_CHANGED.set(true);
        refresh(&mut desktop, 900);
        assert!(requests.try_recv().is_err());
        results.send(Ok(Vec::new())).unwrap();
        refresh(&mut desktop, 901);
        requests.try_recv().unwrap();
        results.send(Ok(Vec::new())).unwrap();
        refresh(&mut desktop, 5_900);
        assert!(requests.try_recv().is_err());
        refresh(&mut desktop, 5_901);
        requests.try_recv().unwrap();
    }
}
