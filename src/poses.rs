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
    Held,
    Wave,
    Sleeve,
    Clap,
    Spin,
    Bored,
    Sit,
}

impl Pose {
    pub const ALL: [Self; 12] = [
        Self::Idle,
        Self::Left,
        Self::Right,
        Self::Rise,
        Self::Fall,
        Self::Held,
        Self::Wave,
        Self::Sleeve,
        Self::Clap,
        Self::Spin,
        Self::Bored,
        Self::Sit,
    ];

    fn sprite(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Left => "walk-left",
            Self::Right => "walk-right",
            Self::Rise => "grab-jump",
            Self::Fall => "grab-jump-air",
            Self::Held => "grab-hold",
            Self::Wave => "wave-cliff",
            Self::Sleeve => "sleeve-garden",
            Self::Clap => "clap-cafe",
            Self::Spin => "battle-spin",
            Self::Bored => "battle-bored",
            Self::Sit => "sit-smile",
        }
    }
}

pub struct PoseClip {
    pub index: usize,
    pub bounds: Bounds,
    frames: Vec<((f64, f64), Frame)>,
}

impl PoseClip {
    pub fn foot(&self, frame: usize) -> (f64, f64) {
        self.frames[frame].0
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
            let id = pose.sprite();
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
                    let visible = frame
                        .visible_bounds()
                        .expect("Missing visible desktop pose");
                    let center_x = visible.center().0;
                    body.point(visible.left - center_x, visible.top - visible.bottom);
                    body.point(visible.right - center_x, 0.0);
                    ((bounds.left + center_x, bounds.top + visible.bottom), frame)
                })
                .collect();
            PoseClip {
                index,
                bounds,
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
    fn every_automatic_frame_is_centered_and_plants_its_last_pixel_above_the_ledge() {
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
                    let columns: Vec<_> = frame
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(_, pixel)| pixel[3] != 0)
                        .map(|(i, _)| i % frame.width)
                        .collect();
                    let center = (*columns.iter().min().unwrap()
                        + *columns.iter().max().unwrap()
                        + 1) as f64
                        / 2.0;
                    let (foot_x, sole) = p.foot(index);
                    assert_eq!(
                        center + (p.bounds.left - foot_x) * scale as f64,
                        0.0,
                        "{pose:?} frame {index} is off-center"
                    );
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
