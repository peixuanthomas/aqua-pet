use super::*;
use crate::{
    ledges::{Ledges, Window},
    wander::Footing,
};
use std::collections::HashSet;
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};

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

unsafe fn read_windows(own: &[HWND]) -> Vec<Window> {
    let mut windows = Vec::new();
    let mut seen = HashSet::new();
    let mut next = GetTopWindow(None);
    while let Ok(hwnd) = next {
        let id = hwnd.0 as usize;
        // Another application may reorder or destroy a window during the walk.
        if !seen.insert(id) {
            break;
        }
        next = GetWindow(hwnd, GW_HWNDNEXT);
        if own.contains(&hwnd) {
            continue;
        }
        let Some(rect) = visible_frame(hwnd) else {
            continue;
        };
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if style & (WS_EX_LAYERED | WS_EX_TRANSPARENT).0 == (WS_EX_LAYERED | WS_EX_TRANSPARENT).0 {
            continue;
        }
        let mut name = [0u16; 128];
        let length = GetClassNameW(hwnd, &mut name) as usize;
        let class = String::from_utf16_lossy(&name[..length]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            continue;
        }
        let can_stand = style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).0 == 0
            && !matches!(class.as_str(), "#32768" | "tooltips_class32");
        windows.push(Window {
            id,
            rect,
            can_stand,
        });
    }
    windows
}

#[derive(Default)]
pub(super) struct Desktop {
    windows: Vec<Window>,
    last_scan: Option<Instant>,
    cached: Option<(Bounds, Bounds, Ledges)>,
}

impl Desktop {
    pub unsafe fn refresh(
        &mut self,
        now: Instant,
        own: &[HWND],
        riding: Option<Footing>,
    ) -> Option<POINT> {
        if self
            .last_scan
            .is_none_or(|t| now.duration_since(t) >= Duration::from_millis(200))
        {
            let windows = read_windows(own);
            if self.windows != windows {
                self.windows = windows;
                self.cached = None;
            }
            self.last_scan = Some(now);
        } else if let Some(s) = riding
            && let Some(i) = self.windows.iter().position(|w| w.id == s.window)
        {
            match visible_frame(HWND(s.window as *mut c_void)) {
                Some(rect) if self.windows[i].rect == rect => {}
                Some(rect) => {
                    self.windows[i].rect = rect;
                    self.cached = None;
                }
                None => {
                    self.windows.remove(i);
                    self.cached = None;
                }
            }
        }
        let s = riding?;
        let rect = self.windows.iter().find(|w| w.id == s.window)?.rect;
        Some(POINT {
            x: (rect.left + s.offset).round() as i32,
            y: rect.top as i32 - 1,
        })
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
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let work = App::work_bounds_at(POINT::default());
            let x = work.left as i32 + 40;
            let y = work.top as i32 + 240;
            let back = Fixture::new(w!("AQUA 测试 · 后窗"), x, y, 580, 200);
            let front = Fixture::new(w!("AQUA 测试 · 前窗"), x + 240, y - 70, 220, 180);
            DwmFlush().unwrap();
            let back_rect = visible_frame(back.0).unwrap();
            let front_rect = visible_frame(front.0).unwrap();
            let mut desktop = Desktop::default();
            let now = Instant::now();
            desktop.refresh(now, &[], None);
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
            let mut roaming = Wander::new(42);
            roaming.set_probability(100);
            roaming.place((world.limits.right, world.limits.bottom));
            let mut jumped = false;
            let mut paced = false;
            for _ in 0..60 * 300 {
                roaming.advance(1.0 / 60.0, 30.0, 1, &fixture_world);
                jumped |= roaming.pose() == Pose::Rise;
                paced |= roaming.riding().is_some()
                    && matches!(roaming.pose(), Pose::Left | Pose::Right);
                if jumped && paced {
                    break;
                }
            }
            assert!(jumped && paced);
            let foot = (back_rect.left + 60.0, back_rect.top);
            let mut pet = Wander::new(42);
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
            desktop.refresh(now + Duration::from_millis(16), &[], pet.riding());
            pet.advance(0.016, 30.0, 1, desktop.ledges(work, body));
            assert_eq!(pet.foot, (foot.0 + 30.0, foot.1 + 20.0));
            let _ = ShowWindow(back.0, SW_SHOWMINNOACTIVE);
            DwmFlush().unwrap();
            desktop.refresh(now + Duration::from_millis(32), &[], pet.riding());
            pet.advance(0.1, 30.0, 1, desktop.ledges(work, body));
            assert!(pet.foot.1 > foot.1 + 20.0);
            assert!(pet.riding().is_none_or(|s| s.window != id));
        }
    }
}
