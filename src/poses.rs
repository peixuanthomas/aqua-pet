use crate::{
    catalog::Catalog,
    render::{self, Bounds},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Idle,
    Left,
    Right,
    Rise,
    Fall,
    Held,
}

impl Pose {
    const ALL: [Self; 6] = [
        Self::Idle,
        Self::Left,
        Self::Right,
        Self::Rise,
        Self::Fall,
        Self::Held,
    ];

    // Keep the planted foot centered when an arm or sleeve extends to one side.
    fn sprite(self) -> (&'static str, f64) {
        match self {
            Self::Idle => ("idle", 9.0),
            Self::Left => ("walk-left", 10.0),
            Self::Right => ("walk-right", 10.0),
            Self::Rise => ("grab-jump", 9.0),
            Self::Fall => ("grab-jump-air", 9.0),
            Self::Held => ("grab-hold", 10.0),
        }
    }
}

pub struct PoseClip {
    pub index: usize,
    pub bounds: Bounds,
    pub foot_x: f64,
    soles: Vec<f64>,
}

impl PoseClip {
    pub fn foot(&self, frame: usize) -> (f64, f64) {
        (self.foot_x, self.soles[frame])
    }
}

pub struct Poses {
    clips: [PoseClip; Pose::ALL.len()],
    body: Bounds,
}

impl Poses {
    pub fn new(catalog: &Catalog) -> Self {
        let mut body = Bounds::empty();
        let clips = Pose::ALL.map(|pose| {
            let (id, foot_x) = pose.sprite();
            let index = catalog
                .clips
                .iter()
                .position(|c| c.id == id)
                .expect("Missing desktop pose");
            let clip = &catalog.clips[index];
            let bounds = render::clip_bounds(catalog, clip, false);
            let soles = clip
                .steps
                .iter()
                .map(|step| {
                    let frame = render::render(catalog, step, bounds, 1.0, false);
                    let mut visible = Bounds::empty();
                    for (i, pixel) in frame.rgba.as_chunks::<4>().0.iter().enumerate() {
                        if pixel[3] != 0 {
                            let x = bounds.left + (i % frame.width) as f64;
                            let y = bounds.top + (i / frame.width) as f64;
                            visible.point(x, y);
                            visible.point(x + 1.0, y + 1.0);
                        }
                    }
                    body.point(visible.left - foot_x, visible.top - visible.bottom);
                    body.point(visible.right - foot_x, 0.0);
                    visible.bottom
                })
                .collect();
            PoseClip {
                index,
                bounds,
                foot_x,
                soles,
            }
        });
        Self { clips, body }
    }

    pub fn get(&self, pose: Pose) -> &PoseClip {
        &self.clips[pose as usize]
    }

    pub fn body(&self, scale: u32) -> Bounds {
        let s = scale as f64;
        Bounds {
            left: self.body.left * s,
            top: self.body.top * s,
            right: self.body.right * s,
            bottom: 0.0,
        }
    }
}
