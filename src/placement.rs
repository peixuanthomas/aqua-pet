use crate::render::Bounds;

/// Keep the whole motion, including its return position, inside a monitor's work area.
/// Coordinates are physical pixels. In particular, monitors left of the primary
/// monitor use negative x; Windows display scaling must not multiply these again.
pub fn fit(
    anchor: (i32, i32),
    requested_scale: u32,
    motion: Bounds,
    work: Bounds,
) -> ((i32, i32), u32) {
    let scale = requested_scale
        .clamp(1, 8)
        .min(((work.width() - 16.0) / motion.width()).floor().max(1.0) as u32)
        .min(((work.height() - 16.0) / motion.height()).floor().max(1.0) as u32);
    let limits = anchor_limits(scale, motion, work);
    (
        (
            anchor.0.clamp(limits.left as i32, limits.right as i32),
            anchor.1.clamp(limits.top as i32, limits.bottom as i32),
        ),
        scale,
    )
}

/// Allowed anchor positions, leaving the entire sprite inside the work area.
pub fn anchor_limits(scale: u32, motion: Bounds, work: Bounds) -> Bounds {
    let s = scale as f64;
    let min_x = work.left as i32 + 8 - (motion.left * s).floor() as i32;
    let max_x = work.right as i32 - 8 - (motion.right * s).ceil() as i32;
    let min_y = work.top as i32 + 8 - (motion.top * s).floor() as i32;
    let max_y = work.bottom as i32 - 8 - (motion.bottom * s).ceil() as i32;
    Bounds {
        left: min_x as f64,
        top: min_y as f64,
        right: max_x.max(min_x) as f64,
        bottom: max_y.max(min_y) as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    #[test]
    fn all_actions_fit_primary_and_negative_coordinate_monitors() {
        let c = Catalog::load().unwrap();
        for (left, top, width, height) in [
            (0.0, 0.0, 1920.0, 1040.0),
            (-1920.0, -200.0, 1920.0, 1040.0),
            (1920.0, 0.0, 2560.0, 1400.0),
            (0.0, 0.0, 1280.0, 680.0),
        ] {
            let work = Bounds {
                left,
                top,
                right: left + width,
                bottom: top + height,
            };
            for clip in &c.clips {
                let motion = crate::render::clip_bounds(&c, clip, true);
                for anchor in [(-10000, -10000), (10000, 10000), (400, 200)] {
                    let ((x, y), scale) = fit(anchor, 4, motion, work);
                    let s = scale as f64;
                    assert!(x as f64 + motion.left * s >= left + 7.0, "{}", clip.id);
                    assert!(y as f64 + motion.top * s >= top + 7.0, "{}", clip.id);
                    assert!(
                        x as f64 + motion.right * s <= work.right - 7.0,
                        "{} too wide",
                        clip.id
                    );
                    assert!(
                        y as f64 + motion.bottom * s <= work.bottom - 7.0,
                        "{} too tall",
                        clip.id
                    );
                }
            }
        }
    }
    #[test]
    fn unchanged_position_does_not_drift_when_refitted() {
        let b = Bounds {
            left: -20.0,
            top: -40.0,
            right: 40.0,
            bottom: 10.0,
        };
        let work = Bounds {
            left: -1920.0,
            top: 0.0,
            right: 0.0,
            bottom: 1040.0,
        };
        let (anchor, scale) = fit((-500, 400), 4, b, work);
        assert_eq!((anchor, scale), ((-500, 400), 4));
        assert_eq!(fit(anchor, 4, b, work), (anchor, scale));
    }
}
