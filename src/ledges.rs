use crate::render::Bounds;

pub const FLOOR: LedgeId = LedgeId::Floor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ControlTop {
    pub left: i32,
    pub right: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgeId {
    Floor,
    Window(usize),
    Control { owner: usize, top: ControlTop },
}

#[derive(Clone, Copy, PartialEq)]
pub struct Window {
    pub id: usize,
    pub rect: Bounds,
    pub can_stand: bool,
}

#[derive(PartialEq)]
pub struct WindowControls {
    pub owner: usize,
    pub size: (f64, f64),
    pub tops: Vec<ControlTop>,
}

impl WindowControls {
    pub fn matches(&self, window: &Window) -> bool {
        self.owner == window.id && self.size == (window.rect.width(), window.rect.height())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub id: LedgeId,
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
    pub fn new(
        work: Bounds,
        body: Bounds,
        windows: &[Window],
        controls: &[WindowControls],
    ) -> Self {
        let limits = Bounds {
            left: work.left - body.left,
            right: (work.right - body.right).max(work.left - body.left),
            top: (work.top - body.top).min(work.bottom),
            bottom: work.bottom,
        };
        let mut edges = vec![Ledge {
            id: FLOOR,
            origin_x: work.left,
            left: limits.left,
            right: limits.right,
            y: work.bottom,
        }];
        for (i, window) in windows.iter().enumerate() {
            let rect = window.rect;
            let mut add_edge = |id, origin_x: f64, right: f64, y| {
                if !(limits.top..work.bottom).contains(&y) {
                    return;
                }
                let is_control = matches!(id, LedgeId::Control { .. });
                let min_width = if is_control { 1.0 } else { body.width() };
                let left = origin_x.max(work.left);
                let right = right.min(work.right);
                if right - left < min_width {
                    return;
                }
                let mut spans = vec![(left, right)];
                for front in &windows[..i] {
                    let cover = front.rect;
                    if cover.top <= y && y < cover.bottom {
                        spans = spans
                            .into_iter()
                            .flat_map(|(a, b)| {
                                [(a, b.min(cover.left)), (a.max(cover.right), b)]
                                    .into_iter()
                                    .filter(|(l, r)| r - l >= min_width)
                            })
                            .collect();
                        if spans.is_empty() {
                            break;
                        }
                    }
                }
                for (a, b) in spans {
                    // A control can support the feet even if the body extends past it.
                    let (left, right) = if is_control {
                        (a.max(limits.left), b.min(limits.right))
                    } else {
                        (a - body.left, b - body.right)
                    };
                    if left <= right {
                        edges.push(Ledge {
                            id,
                            origin_x,
                            left,
                            right,
                            y,
                        });
                    }
                }
            };
            if window.can_stand {
                add_edge(LedgeId::Window(window.id), rect.left, rect.right, rect.top);
            }
            if let Some(controls) = controls.iter().find(|controls| controls.matches(window)) {
                for &top in &controls.tops {
                    add_edge(
                        LedgeId::Control {
                            owner: window.id,
                            top,
                        },
                        rect.left + f64::from(top.left),
                        rect.left + f64::from(top.right),
                        rect.top + f64::from(top.y),
                    );
                }
            }
        }
        Self { limits, edges }
    }

    pub fn at_offset(&self, id: LedgeId, offset: f64) -> Option<Ledge> {
        self.edges
            .iter()
            .copied()
            .find(|e| e.id == id && e.contains(e.origin_x + offset))
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
            let world = Ledges::new(work, body, &windows, &[]);
            let edges: Vec<_> = world
                .edges
                .iter()
                .map(|e| (e.id, e.left, e.right))
                .collect();
            assert_eq!(
                edges,
                vec![
                    (FLOOR, work.left - body.left, -body.right),
                    (
                        LedgeId::Window(2),
                        work.left - body.left,
                        -800.0 - body.right
                    ),
                    (LedgeId::Window(2), -600.0 - body.left, -110.0 - body.right)
                ]
            );
        }
    }

    #[test]
    fn control_tops_allow_narrow_spans_and_need_room_above_them() {
        let work = rect(-600.0, -100.0, 0.0, 640.0);
        let body = rect(-25.0, -60.0, 25.0, 0.0);
        let mut windows = [
            Window {
                id: 1,
                rect: rect(-420.0, -20.0, -350.0, 380.0),
                can_stand: false,
            },
            Window {
                id: 2,
                rect: rect(-600.0, -100.0, 0.0, 540.0),
                can_stand: true,
            },
        ];
        let narrow = ControlTop {
            left: 80,
            right: 98,
            y: 110,
        };
        let split = ControlTop {
            left: 130,
            right: 300,
            y: 180,
        };
        let controls = [WindowControls {
            owner: 2,
            size: (600.0, 640.0),
            tops: vec![
                ControlTop {
                    left: 80,
                    right: 200,
                    y: 30,
                },
                narrow,
                ControlTop {
                    left: 200,
                    right: 240,
                    y: 110,
                },
                split,
            ],
        }];
        let world = Ledges::new(work, body, &windows, &controls);
        let edges: Vec<_> = world.edges[1..]
            .iter()
            .map(|edge| (edge.id, edge.left, edge.right, edge.y))
            .collect();
        assert_eq!(
            edges,
            [
                (
                    LedgeId::Control {
                        owner: 2,
                        top: narrow
                    },
                    -520.0,
                    -502.0,
                    10.0
                ),
                (
                    LedgeId::Control {
                        owner: 2,
                        top: split
                    },
                    -470.0,
                    -420.0,
                    80.0
                ),
                (
                    LedgeId::Control {
                        owner: 2,
                        top: split
                    },
                    -350.0,
                    -300.0,
                    80.0
                ),
            ]
        );
        windows[1].rect.right -= 20.0;
        assert_eq!(Ledges::new(work, body, &windows, &controls).edges.len(), 1);
    }
}
