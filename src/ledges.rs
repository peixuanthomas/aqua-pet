use crate::render::Bounds;

pub const FLOOR: usize = 0;

#[derive(Clone, Copy, PartialEq)]
pub struct Window {
    pub id: usize,
    pub rect: Bounds,
    pub can_stand: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub window: usize,
    pub origin_x: f64,
    pub left: f64,
    pub right: f64,
    pub y: f64,
}

impl Ledge {
    pub fn contains(self, x: f64) -> bool {
        x >= self.left - 1e-7 && x <= self.right + 1e-7
    }
}

pub struct Ledges {
    pub limits: Bounds,
    pub edges: Vec<Ledge>,
}

impl Ledges {
    pub fn new(work: Bounds, body: Bounds, windows: &[Window]) -> Self {
        let limits = Bounds {
            left: work.left - body.left,
            right: (work.right - body.right).max(work.left - body.left),
            top: (work.top - body.top).min(work.bottom),
            bottom: work.bottom,
        };
        let mut edges = vec![Ledge {
            window: FLOOR,
            origin_x: work.left,
            left: limits.left,
            right: limits.right,
            y: work.bottom,
        }];
        for (i, window) in windows.iter().enumerate() {
            let rect = window.rect;
            if !window.can_stand || !(limits.top..work.bottom).contains(&rect.top) {
                continue;
            }
            let left = rect.left.max(work.left);
            let right = rect.right.min(work.right);
            if right - left < body.width() {
                continue;
            }
            let mut spans = vec![(left, right)];
            for front in &windows[..i] {
                let cover = front.rect;
                if cover.top <= rect.top && rect.top < cover.bottom {
                    spans = spans
                        .into_iter()
                        .flat_map(|(a, b)| {
                            [(a, b.min(cover.left)), (a.max(cover.right), b)]
                                .into_iter()
                                .filter(|(l, r)| r - l >= body.width())
                        })
                        .collect();
                }
            }
            edges.extend(spans.into_iter().map(|(a, b)| Ledge {
                window: window.id,
                origin_x: rect.left,
                left: a - body.left,
                right: b - body.right,
                y: rect.top,
            }));
        }
        Self { limits, edges }
    }

    pub fn at_offset(&self, window: usize, offset: f64) -> Option<Ledge> {
        self.edges
            .iter()
            .copied()
            .find(|e| e.window == window && e.contains(e.origin_x + offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Bounds {
        Bounds {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn uncovered_intervals_fit_the_whole_pet_on_a_negative_coordinate_monitor() {
        let work = rect(-1200.0, -200.0, 0.0, 700.0);
        let windows = [
            Window {
                id: 1,
                rect: rect(-800.0, -190.0, -600.0, 300.0),
                can_stand: false,
            },
            Window {
                id: 2,
                rect: rect(-1300.0, 80.0, -110.0, 500.0),
                can_stand: true,
            },
            Window {
                id: 3,
                rect: rect(-500.0, -200.0, -210.0, 100.0),
                can_stand: true,
            },
        ];
        for scale in [1.0, 2.0, 4.0, 8.0] {
            let body = rect(-10.0 * scale, -35.0 * scale, 10.0 * scale, 0.0);
            let world = Ledges::new(work, body, &windows);
            let edges: Vec<_> = world
                .edges
                .iter()
                .map(|e| (e.window, e.left, e.right))
                .collect();
            assert_eq!(
                edges,
                vec![
                    (FLOOR, work.left - body.left, -body.right),
                    (2, work.left - body.left, -800.0 - body.right),
                    (2, -600.0 - body.left, -110.0 - body.right)
                ]
            );
        }
    }
}
