use crate::{
    catalog::Catalog,
    render::{self, Bounds, Frame},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Idle,
    Left,
    Right,
    Rise,
    Fall,
    Wave,
    Sleeve,
    Clap,
    Spin,
    Dance,
    CliffSpin,
    Bored,
    Sit,
}

impl Pose {
    pub const ALL: [Self; 13] = [
        Self::Idle,
        Self::Left,
        Self::Right,
        Self::Rise,
        Self::Fall,
        Self::Wave,
        Self::Sleeve,
        Self::Clap,
        Self::Spin,
        Self::Dance,
        Self::CliffSpin,
        Self::Bored,
        Self::Sit,
    ];

    // Keep the planted foot centered when an arm or sleeve extends to one side.
    fn sprite(self) -> (&'static str, f64) {
        match self {
            Self::Idle => ("idle", 9.0),
            Self::Left => ("walk-left", 10.0),
            Self::Right => ("walk-right", 10.0),
            Self::Rise => ("grab-jump", 9.0),
            Self::Fall => ("grab-jump-air", 9.0),
            Self::Wave => ("wave-cliff", 12.0),
            Self::Sleeve => ("sleeve-garden", 7.0),
            Self::Clap => ("clap-cafe", 10.0),
            Self::Spin => ("battle-spin", 10.0),
            Self::Dance => ("battle-dance", 9.0),
            Self::CliffSpin => ("spin-cliff", 10.0),
            Self::Bored => ("battle-bored", 9.0),
            Self::Sit => ("sit-smile", -2.0),
        }
    }
}

pub struct PoseClip {
    pub index: usize,
    pub bounds: Bounds,
    pub foot_x: f64,
    frames: Vec<(f64, Frame)>,
}

impl PoseClip {
    pub fn foot(&self, frame: usize) -> (f64, f64) {
        (self.foot_x, self.frames[frame].0)
    }

    pub fn frame(&self, index: usize) -> &Frame {
        &self.frames[index].1
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
            let frames = clip
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
                    (visible.bottom, frame)
                })
                .collect();
            PoseClip {
                index,
                bounds,
                foot_x,
                frames,
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

    pub fn durations(&self, catalog: &Catalog) -> [f64; Pose::ALL.len()] {
        self.clips
            .each_ref()
            .map(|c| catalog.clips[c.index].duration() / catalog.game_fps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_automatic_frame_plants_its_last_visible_pixel_above_the_ledge() {
        let catalog = Catalog::load().unwrap();
        let poses = Poses::new(&catalog);
        for pose in Pose::ALL {
            let p = poses.get(pose);
            for (index, step) in catalog.clips[p.index].steps.iter().enumerate() {
                for scale in 1..=8 {
                    let frame = render::render(&catalog, step, p.bounds, scale as f64, false);
                    let mut expected = vec![[0; 4]; frame.width * frame.height];
                    let mut scaled = vec![[255; 4]; expected.len()];
                    frame.write_bgra(&mut expected, 1);
                    p.frame(index).write_bgra(&mut scaled, scale);
                    assert!(scaled == expected, "{pose:?} frame {index} at {scale}x");
                    let bottom = frame
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .rchunks(frame.width)
                        .position(|row| row.iter().any(|px| px[3] != 0))
                        .unwrap();
                    let painted = frame.height - bottom;
                    let (_, sole) = p.foot(index);
                    assert_eq!(
                        painted as f64 + (p.bounds.top - sole) * scale as f64,
                        0.0,
                        "{pose:?} frame {index}"
                    );
                }
            }
        }
    }
}
