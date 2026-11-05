use super::*;

fn assert_centered(position: POINT, cursor: POINT, visible: Bounds, scale: u32) {
    let (cx, cy) = visible.center();
    assert!((position.x as f64 + cx * scale as f64 - cursor.x as f64).abs() <= 0.5);
    assert!((position.y as f64 + cy * scale as f64 - cursor.y as f64).abs() <= 0.5);
}

#[test]
fn all_resource_and_action_frames_stay_centered_while_dragging() {
    let mut app = App::new(
        Catalog::load().unwrap(),
        HWND::default(),
        HWND::default(),
        HWND::default(),
        true,
    );
    app.debug_open = true;
    for clip_index in 0..app.catalog.clips.len() {
        let clip = &app.catalog.clips[clip_index];
        app.player.select(clip_index, clip);
        app.bounds = render::clip_bounds(&app.catalog, clip, false);
        for index in 0..clip.steps.len() {
            let clip = &app.catalog.clips[clip_index];
            let frame = render::render(&app.catalog, &clip.steps[index], app.bounds, 1.0, false);
            app.player.tick = clip.step_start(index);
            let (cx, cy) = frame.center();
            let visible = frame.visible_bounds();
            for scale in 1..=8 {
                app.effective_scale = scale;
                app.frame_center = (cx * scale as f64, cy * scale as f64);
                for cursor in [POINT { x: 600, y: 400 }, POINT { x: -1700, y: -100 }] {
                    app.anchor = POINT { x: 300, y: 200 };
                    app.drag = Some(cursor);
                    let position = app.pet_position(index);
                    if let Some(visible) = visible {
                        assert_centered(position, cursor, visible, scale);
                    }
                    app.drag = None;
                    assert_eq!(app.pet_position(index), position);
                }
            }
        }
    }
}

#[test]
fn automatic_frames_release_from_the_centered_foot_position() {
    let mut app = App::new(
        Catalog::load_desktop().unwrap(),
        HWND::default(),
        HWND::default(),
        HWND::default(),
        true,
    );
    for pose in Pose::ALL {
        app.show_pose(pose);
        let count = app.catalog.clips[app.player.selected].steps.len();
        for index in 0..count {
            let frame = app.poses.get(pose).frame(index);
            let visible = frame.visible_bounds().unwrap();
            let (cx, cy) = frame.center();
            let (ix, iy) = app.poses.get(Pose::Idle).foot(0);
            for scale in 1..=8 {
                app.effective_scale = scale;
                app.frame_center = (cx * scale as f64, cy * scale as f64);
                let cursor = POINT { x: -700, y: 400 };
                app.drag = Some(cursor);
                let position = app.pet_position(index);
                assert_centered(position, cursor, visible, scale);
                let foot = (
                    app.anchor.x as f64 + ix * scale as f64,
                    app.anchor.y as f64 + iy * scale as f64,
                );
                assert!((foot.0 - cursor.x as f64).abs() <= 0.5);
                assert!((foot.1 - position.y as f64 - visible.bottom * scale as f64).abs() <= 0.5);
                app.drag = None;
                assert_eq!(app.pet_position(index), position);
            }
        }
    }
}
