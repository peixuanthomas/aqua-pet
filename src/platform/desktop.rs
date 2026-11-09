use super::*;
use crate::{
    ledges::{ControlTop, LedgeId, Ledges, Window, WindowControls},
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
use windows::Win32::UI::Accessibility::{
    HWINEVENTHOOK, SetWinEventHook, TreeScope_Subtree, UnhookWinEvent,
};

const SCAN_INTERVAL: Duration = Duration::from_millis(200);
const CONTROL_SCAN_INTERVAL: Duration = Duration::from_millis(500);
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

fn cursor_approached(previous: POINT, current: POINT, foot: (f64, f64), body: Bounds) -> bool {
    let near = |point: POINT| {
        let (x, y) = (point.x as f64 - foot.0, point.y as f64 - foot.1);
        (x - x.clamp(body.left, body.right)).hypot(y - y.clamp(body.top, body.bottom))
            <= body.width()
    };
    // Compare both cursor positions against the same pet position.
    near(current) && !near(previous)
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

struct ControlScan {
    requests: Sender<Vec<Window>>,
    snapshots: Receiver<Vec<WindowControls>>,
    next_scan: Instant,
    pending: bool,
}

impl ControlScan {
    fn new() -> Self {
        let (requests, read_requests) = mpsc::channel::<Vec<Window>>();
        let (results, snapshots) = mpsc::channel();
        uia::spawn_reader(TreeScope_Subtree, move |reader| unsafe {
            while let Ok(windows) = read_requests.recv() {
                let snapshot = windows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, window)| {
                        let hwnd = HWND(window.id as *mut c_void);
                        let frame = visible_frame(hwnd)?;
                        if windows[..index].iter().any(|front| {
                            front.rect.left <= frame.left
                                && front.rect.top <= frame.top
                                && front.rect.right >= frame.right
                                && front.rect.bottom >= frame.bottom
                        }) {
                            return None;
                        }
                        let rect = RECT {
                            left: frame.left as i32,
                            top: frame.top as i32,
                            right: frame.right as i32,
                            bottom: frame.bottom as i32,
                        };
                        let controls = reader.window_controls(hwnd, rect).ok()?;
                        if controls.is_empty() || visible_frame(hwnd) != Some(frame) {
                            return None;
                        }
                        let mut tops: Vec<_> = controls
                            .into_iter()
                            .map(|control| ControlTop {
                                left: control.left - rect.left,
                                right: control.right - rect.left,
                                y: control.top - rect.top,
                            })
                            .collect();
                        tops.sort_unstable();
                        tops.dedup();
                        Some(WindowControls {
                            owner: window.id,
                            size: (frame.width(), frame.height()),
                            tops,
                        })
                    })
                    .collect();
                if results.send(snapshot).is_err() {
                    break;
                }
            }
            Ok(())
        });
        Self {
            requests,
            snapshots,
            next_scan: Instant::now(),
            pending: false,
        }
    }

    fn refresh(&mut self, now: Instant, windows: &[Window]) -> Option<Vec<WindowControls>> {
        let snapshot = self.snapshots.try_recv().ok();
        if snapshot.is_some() {
            self.pending = false;
            self.next_scan = now + CONTROL_SCAN_INTERVAL;
        }
        if !self.pending && now >= self.next_scan && !windows.is_empty() {
            self.next_scan = now + CONTROL_SCAN_INTERVAL;
            self.pending = self.requests.send(windows.to_vec()).is_ok();
        }
        snapshot
    }
}

pub(super) struct Desktop {
    windows: Vec<Window>,
    controls: Vec<WindowControls>,
    control_scan: Option<ControlScan>,
    hooks: [HWINEVENTHOOK; 5],
    rescan_interval: Duration,
    last_scan: Instant,
    request: Sender<()>,
    snapshots: Receiver<windows::core::Result<Vec<Window>>>,
    scanning: bool,
    cached: Option<(Bounds, Bounds, Ledges)>,
    cursor: Option<POINT>,
}

impl Desktop {
    pub fn new(own: &[HWND], inspect_controls: bool) -> Self {
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
            controls: Vec::new(),
            control_scan: inspect_controls.then(ControlScan::new),
            hooks,
            rescan_interval,
            last_scan: Instant::now(),
            request,
            snapshots,
            scanning: false,
            cached: None,
            cursor: None,
        }
    }

    pub fn activity(&mut self, foot: (f64, f64), body: Bounds) -> (bool, bool) {
        let mut point = POINT::default();
        let current = unsafe { GetCursorPos(&mut point) }.is_ok().then_some(point);
        let previous = std::mem::replace(&mut self.cursor, current);
        let approached = previous
            .zip(current)
            .is_some_and(|(previous, current)| cursor_approached(previous, current, foot, body));
        let mut last = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            ..Default::default()
        };
        let idle = unsafe {
            GetLastInputInfo(&mut last).as_bool()
                && idle_for_two_minutes(GetTickCount(), last.dwTime)
        };
        (approached, idle)
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
                self.controls
                    .retain(|controls| self.windows.iter().any(|window| controls.matches(window)));
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
        if let Some(mut controls) = self
            .control_scan
            .as_mut()
            .and_then(|scan| scan.refresh(now, &self.windows))
        {
            controls.retain(|controls| self.windows.iter().any(|window| controls.matches(window)));
            if self.controls != controls {
                self.controls = controls;
                self.cached = None;
            }
        }
        let s = riding?;
        let (owner, control) = match s.id {
            LedgeId::Floor => return None,
            LedgeId::Window(id) => (id, None),
            LedgeId::Control { owner, top } => (owner, Some(top)),
        };
        let i = self.windows.iter().position(|w| w.id == owner)?;
        let hwnd = HWND(owner as *mut c_void);
        // A window can move again between the background read and this animation frame.
        let rect = if changed || self.scanning || received || self.rescan_interval == SCAN_INTERVAL
        {
            visible_frame(hwnd)
        } else {
            Some(self.windows[i].rect)
        };
        // Minimize can happen before the notification or while the frame is being read.
        let Some(rect) =
            rect.filter(|_| IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool())
        else {
            self.windows.remove(i);
            self.cached = None;
            return None;
        };
        if self.windows[i].rect != rect {
            self.windows[i].rect = rect;
            self.cached = None;
        }
        if let Some(top) = control {
            self.controls.iter().find(|controls| {
                controls.matches(&self.windows[i]) && controls.tops.contains(&top)
            })?;
            return Some((
                rect.left + f64::from(top.left) + s.offset,
                rect.top + f64::from(top.y),
            ));
        }
        Some((rect.left + s.offset, rect.top))
    }

    pub fn ledges(&mut self, work: Bounds, body: Bounds) -> &Ledges {
        if self
            .cached
            .as_ref()
            .is_none_or(|(w, b, _)| *w != work || *b != body)
        {
            self.cached = Some((
                work,
                body,
                Ledges::new(work, body, &self.windows, &self.controls),
            ));
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
                for seated in [false, true] {
                    check_windows(App::work_bounds_at(center), seated);
                }
            }
        }
    }

    unsafe fn check_windows(work: Bounds, seated: bool) {
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
        let mut desktop = Desktop::new(&[], false);
        let id = back.0.0 as usize;
        let front_id = front.0.0 as usize;
        // The user's other open apps should not decide which test window wins.
        desktop.windows.retain(|w| [id, front_id].contains(&w.id));
        let now = Instant::now();
        desktop.refresh(now, None);
        let body = Bounds {
            left: -18.0,
            top: -36.0,
            right: 18.0,
            bottom: 0.0,
        };
        let world = desktop.ledges(work, body);
        let back_edges: Vec<_> = world
            .edges
            .iter()
            .filter(|e| e.id == LedgeId::Window(id))
            .collect();
        assert_eq!(back_edges.len(), 2);
        assert!(back_edges[0].right < front_rect.left && back_edges[1].left > front_rect.right);
        let front_edge = world
            .edges
            .iter()
            .find(|e| e.id == LedgeId::Window(front_id))
            .unwrap();
        assert_ne!(GetForegroundWindow(), back.0);
        let mut roaming = Wander::for_test(42);
        roaming.set_probability(100);
        roaming.place((front_edge.left, front_edge.y));
        let mut jumping_from_front = false;
        let mut reached_back = false;
        for _ in 0..60 * 300 {
            let preparing = matches!(roaming.pose(), Pose::LookLeft | Pose::LookRight)
                && roaming
                    .riding()
                    .is_some_and(|s| s.id == LedgeId::Window(front_id));
            roaming.advance(1.0 / 60.0, 30.0, 1, world);
            jumping_from_front |= preparing && roaming.support.is_none();
            if jumping_from_front && let Some(support) = roaming.support {
                if support.id == LedgeId::Window(id) {
                    reached_back = true;
                    break;
                }
                jumping_from_front = false;
            }
        }
        assert!(reached_back);
        let foot = (back_rect.left + 60.0, back_rect.top);
        let mut pet = Wander::for_test(42);
        pet.set_probability(0);
        pet.set_music_playing(true);
        pet.place((foot.0, foot.1 - 50.0));
        pet.advance(0.7, 30.0, 1, world);
        assert_eq!(pet.foot, foot);
        assert_eq!(pet.riding().unwrap().id, LedgeId::Window(id));
        assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));
        if seated {
            pet.observe(false, true, 0.01);
            pet.advance(0.016, 30.0, 1, world);
            assert_eq!(pet.pose(), Pose::Perch);
            pet.observe(true, false, 0.01);
            assert_eq!(pet.pose(), Pose::PerchWave);
        }
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
        if seated {
            assert_eq!(pet.pose(), Pose::PerchWave);
        }
        let _ = ShowWindow(back.0, SW_SHOWMINNOACTIVE);
        DwmFlush().unwrap();
        assert!(IsIconic(back.0).as_bool());
        // The pet should fall even if Windows has not delivered the minimize event yet.
        WINDOWS_CHANGED.set(false);
        desktop.rescan_interval = Duration::from_secs(5);
        desktop.refresh(now + Duration::from_millis(32), pet.riding());
        let world = desktop.ledges(work, body);
        pet.advance(0.1, 30.0, 1, world);
        assert_eq!(pet.pose(), Pose::Fall);
        assert!(pet.foot.1 > foot.1 + 20.0);
        for _ in 0..60 {
            pet.advance(0.1, 30.0, 1, world);
            if pet.foot.1 == work.bottom {
                break;
            }
        }
        assert_eq!(pet.foot, (foot.0 + 30.0, work.bottom));
        pet.advance(0.016, 30.0, 1, world);
        assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));
    }

    #[test]
    #[ignore = "reads a child button in a real Win32 window"]
    fn live_button_top_moves_with_its_window_and_disappears_when_hidden() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let work = App::work_bounds_at(POINT::default());
            let x = work.left as i32 + 80;
            let y = work.top as i32 + 160;
            let window = Fixture::new(w!("AQUA 控件测试"), x, y, 340, 230);
            let button = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Test"),
                WS_CHILD | WS_VISIBLE,
                80,
                60,
                28,
                22,
                Some(window.0),
                None,
                None,
                None,
            )
            .unwrap();
            DwmFlush().unwrap();
            let frame = visible_frame(window.0).unwrap();
            let mut bounds = RECT::default();
            GetWindowRect(button, &mut bounds).unwrap();
            let owner = window.0.0 as usize;
            let id = LedgeId::Control {
                owner,
                top: ControlTop {
                    left: bounds.left - frame.left as i32,
                    right: bounds.right - frame.left as i32,
                    y: bounds.top - frame.top as i32,
                },
            };
            let body = Bounds {
                left: -35.0,
                top: -55.0,
                right: 35.0,
                bottom: 0.0,
            };
            let mut desktop = Desktop::new(&[], true);
            desktop.windows.retain(|window| window.id == owner);
            WINDOWS_CHANGED.set(false);
            let wait_for_ledge = |desktop: &mut Desktop, present| {
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let mut message = MSG::default();
                    while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                        DispatchMessageW(&message);
                    }
                    desktop.refresh(Instant::now(), None);
                    if desktop
                        .ledges(work, body)
                        .edges
                        .iter()
                        .any(|edge| edge.id == id)
                        == present
                    {
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "unexpected button ledge visibility"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
            };
            wait_for_ledge(&mut desktop, true);
            let mut pet = Wander::for_test(7);
            pet.set_probability(0);
            let foot = (f64::from(bounds.left + 14), f64::from(bounds.top));
            pet.place((foot.0, foot.1 - 10.0));
            pet.advance(0.2, 30.0, 1, desktop.ledges(work, body));
            assert_eq!(pet.riding().unwrap().id, id);

            SetWindowPos(
                window.0,
                None,
                x + 36,
                y + 24,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .unwrap();
            DwmFlush().unwrap();
            WINDOWS_CHANGED.set(true);
            desktop.refresh(Instant::now(), pet.riding());
            pet.advance(0.016, 30.0, 1, desktop.ledges(work, body));
            assert_eq!(pet.foot, (foot.0 + 36.0, foot.1 + 24.0));

            let _ = ShowWindow(button, SW_HIDE);
            wait_for_ledge(&mut desktop, false);
            pet.advance(0.1, 30.0, 1, desktop.ledges(work, body));
            assert!(pet.riding().is_none());
            assert!(pet.foot.1 > foot.1 + 24.0);
        }
    }

    #[test]
    fn a_pet_moving_into_range_of_a_still_cursor_is_not_an_approach() {
        let body = Bounds {
            left: -15.0,
            right: 15.0,
            top: -36.0,
            bottom: 0.0,
        };
        let foot = (190.0, 900.0);
        let far = POINT { x: 310, y: 880 };
        let near = POINT { x: 220, y: 880 };
        assert!(!cursor_approached(far, near, (100.0, 900.0), body));
        assert!(!cursor_approached(near, near, foot, body));
        assert!(!cursor_approached(near, far, foot, body));
        assert!(cursor_approached(far, near, foot, body));
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
            controls: Vec::new(),
            control_scan: None,
            hooks: [HWINEVENTHOOK::default(); 5],
            rescan_interval: Duration::from_secs(5),
            last_scan: now,
            request,
            snapshots,
            scanning: false,
            cached: None,
            cursor: None,
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
