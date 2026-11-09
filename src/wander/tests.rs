use super::*;
use crate::{catalog::Catalog, poses::Poses};
use crate::{ledges::Window, render::Bounds};
use std::sync::OnceLock;

impl Wander {
    pub(crate) fn for_test(seed: u64) -> Self {
        static DURATIONS: OnceLock<[f64; Pose::ALL.len()]> = OnceLock::new();
        let durations = DURATIONS.get_or_init(|| {
            let catalog = Catalog::load().unwrap();
            Poses::new(&catalog).durations(&catalog)
        });
        Self::new(seed, *durations)
    }
}

fn window(id: usize, left: f64, top: f64, right: f64, bottom: f64) -> Window {
    Window {
        id,
        rect: Bounds {
            left,
            top,
            right,
            bottom,
        },
        can_stand: true,
    }
}

fn desktop(windows: &[Window]) -> Ledges {
    Ledges::new(
        window(0, 0.0, 0.0, 1200.0, 900.0).rect,
        window(0, -15.0, -36.0, 15.0, 0.0).rect,
        windows,
    )
}

fn grounded(world: &Ledges, x: f64) -> Wander {
    let mut pet = Wander::for_test(42);
    pet.place((x, world.limits.bottom));
    pet.advance(0.01, 30.0, 1, world);
    pet
}

#[test]
fn a_drag_drop_claps_three_times_after_landing_then_sways() {
    let world = desktop(&[window(1, 250.0, 300.0, 800.0, 600.0)]);
    for (x, edge) in [(100.0, world.edges[0]), (300.0, world.edges[1])] {
        let mut pet = Wander::for_test(42);
        pet.set_music_playing(true);
        pet.drop_at((x, edge.y - 45.0), true);
        assert_eq!(pet.pose(), Pose::Fall);
        pet.advance((90.0_f64 / 900.0).sqrt(), 30.0, 1, &world);
        assert_eq!(pet.support.unwrap().window, edge.window);
        assert_eq!(pet.foot, (x, edge.y));
        assert_eq!(pet.pose(), Pose::Clap);
        pet.observe(true, false, 0.01);
        pet.advance(0.99, 30.0, 1, &world);
        assert_eq!(pet.pose(), Pose::Clap);
        pet.advance(0.01, 30.0, 1, &world);
        assert_eq!(pet.pose(), Pose::Idle);
        pet.advance(1.6, 30.0, 1, &world);
        assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));
    }
}

#[test]
fn a_regular_fall_after_a_drag_does_not_clap_again() {
    let world = desktop(&[]);
    let mut pet = Wander::for_test(42);
    pet.drop_at((100.0, 800.0), true);
    pet.place((100.0, 800.0));
    pet.advance(0.5, 30.0, 1, &world);
    assert_eq!(pet.foot, (100.0, world.limits.bottom));
    assert_eq!(pet.pose(), Pose::Idle);
}

fn neighboring_windows(scale: u32, gap: f64, height: f64) -> Ledges {
    let s = scale as f64;
    Ledges::new(
        window(0, 0.0, 0.0, 1200.0 * s, 900.0 * s).rect,
        window(0, -15.0 * s, -36.0 * s, 15.0 * s, 0.0).rect,
        &[
            window(1, 100.0 * s, 350.0 * s, 400.0 * s, 800.0 * s),
            window(
                2,
                (400.0 + gap) * s,
                (350.0 + height) * s,
                900.0 * s,
                850.0 * s,
            ),
        ],
    )
}

#[test]
fn window_hops_play_roll_once_and_land_on_time_at_every_scale_and_timer_rate() {
    for scale in [1, 2, 4, 8] {
        for height in [-35.0, 0.0, 35.0] {
            let world = neighboring_windows(scale, 20.0, height);
            for right in [false, true] {
                let (edge, target, from, to) = if right {
                    (
                        world.edges[1],
                        world.edges[2],
                        world.edges[1].right,
                        world.edges[2].left,
                    )
                } else {
                    (
                        world.edges[2],
                        world.edges[1],
                        world.edges[2].left,
                        world.edges[1].right,
                    )
                };
                for rate in [10, 30, 60, 144] {
                    let mut pet = Wander::for_test(42);
                    pet.set_probability(0);
                    pet.place((from, edge.y));
                    pet.advance(0.01, 30.0, scale, &world);
                    pet.jump(
                        edge,
                        Footing::new(target, to),
                        &world,
                        scale as f64,
                        60.0 * scale as f64,
                    );
                    assert!(pet.support.is_none());
                    assert_eq!(pet.pose(), Pose::Rise);
                    let rise = pet.pose_seconds[Pose::Rise as usize];
                    let roll = pet.pose_seconds[Pose::Roll as usize];
                    let duration = rise + roll;
                    let dt = 1.0 / rate as f64;
                    let mut elapsed = 0.0;
                    while elapsed + dt < duration - 1e-9 {
                        pet.advance(dt, 30.0, scale, &world);
                        elapsed += dt;
                        assert!(pet.support.is_none(), "landed early at {elapsed}");
                        assert!(pet.foot.1 >= world.limits.top);
                        if elapsed + 1e-9 >= rise {
                            assert_eq!(pet.pose(), Pose::Roll);
                            assert!((pet.animation_seconds() - (elapsed - rise)).abs() < 1e-8);
                            assert!(pet.animation_seconds() < roll);
                        }
                    }
                    pet.advance(duration - elapsed, 30.0, scale, &world);
                    assert_eq!(pet.riding().unwrap().window, target.window);
                    assert!((pet.foot.0 - to).abs() < 1e-7);
                    assert_eq!(pet.foot.1, target.y);
                    assert_eq!(pet.pose(), Pose::Idle);
                }
            }
        }
    }
}

#[test]
fn window_hops_reject_excessive_horizontal_or_vertical_distance() {
    for scale in [1, 2, 4, 8] {
        for (gap, height) in [(20.1, 0.0), (20.0, -35.1), (20.0, 35.1)] {
            let world = neighboring_windows(scale, gap, height);
            let edge = world.edges[1];
            let target = world.edges[2];
            let mut pet = Wander::for_test(42);
            pet.place((edge.right, edge.y));
            pet.advance(0.01, 30.0, scale, &world);
            pet.jump(
                edge,
                Footing::new(target, target.left),
                &world,
                scale as f64,
                60.0 * scale as f64,
            );
            assert_eq!(pet.riding().unwrap().window, edge.window);
            assert_eq!(pet.pose(), Pose::Idle);
        }
    }
}

#[test]
fn nearby_window_is_chosen_and_approached_before_hopping() {
    let world = neighboring_windows(1, 20.0, -35.0);
    let edge = world.edges[1];
    let target = world.edges[2];
    for seed in 1..=100 {
        let mut pet = Wander::for_test(seed);
        pet.set_probability(100);
        pet.place((edge.left, edge.y));
        pet.advance(0.01, 30.0, 1, &world);
        pet.choose(edge, &world, 60.0, 1.0);
        if matches!(pet.motion, Motion::Walk { jump_to: Some(s), .. } if s.window == target.window)
        {
            let mut rolled = false;
            for _ in 0..60 * 6 {
                pet.advance(1.0 / 60.0, 30.0, 1, &world);
                rolled |= pet.pose() == Pose::Roll;
                if pet.riding().is_some_and(|s| s.window == target.window) {
                    assert!(rolled);
                    return;
                }
            }
            panic!("the selected window was not reached");
        }
    }
    panic!("the neighboring window was never chosen");
}

#[test]
fn far_or_covered_windows_are_not_selected_for_a_hop() {
    let source = window(1, 100.0, 350.0, 400.0, 800.0);
    let target = window(2, 420.0, 350.0, 900.0, 850.0);
    let mut cover = window(3, 410.0, 300.0, 910.0, 860.0);
    cover.can_stand = false;
    let worlds = [
        neighboring_windows(1, 21.0, 0.0),
        neighboring_windows(1, 20.0, 36.0),
        neighboring_windows(1, 20.0, -36.0),
        desktop(&[cover, source, target]),
    ];
    for world in worlds {
        let edge = *world.edges.iter().find(|e| e.window == 1).unwrap();
        for seed in 1..=100 {
            let mut pet = Wander::for_test(seed);
            pet.set_probability(100);
            pet.place((edge.left, edge.y));
            pet.advance(0.01, 30.0, 1, &world);
            pet.choose(edge, &world, 60.0, 1.0);
            assert!(!matches!(pet.motion, Motion::Air { roll: true, .. }));
            if let Motion::Walk {
                jump_to: Some(target),
                ..
            } = pet.motion
            {
                assert_eq!(target.window, FLOOR);
            }
        }
    }
}

#[test]
fn approaching_pet_rechecks_the_target_before_leaving_its_window() {
    let world = neighboring_windows(1, 20.0, 0.0);
    let edge = world.edges[1];
    let target = Footing::new(world.edges[2], world.edges[2].left);
    for changed in [
        neighboring_windows(1, 21.0, 0.0),
        neighboring_windows(1, 20.0, 36.0),
        desktop(&[window(1, 100.0, 350.0, 400.0, 800.0)]),
    ] {
        let mut pet = Wander::for_test(42);
        pet.place((edge.right - 1.0, edge.y));
        pet.advance(0.01, 30.0, 1, &world);
        pet.walk_to(edge, edge.right, Some(target));
        pet.advance(0.1, 30.0, 1, &changed);
        assert_eq!(pet.riding().unwrap().window, edge.window);
        assert_eq!(pet.pose(), Pose::Idle);
        assert_eq!(pet.foot, (edge.right, edge.y));
    }
}

#[test]
fn a_missed_window_ends_the_roll_and_falls_normally() {
    let world = neighboring_windows(1, 20.0, 0.0);
    let edge = world.edges[1];
    let target = Footing::new(world.edges[2], world.edges[2].left);
    let source = window(1, 100.0, 350.0, 400.0, 800.0);
    let mut cover = window(3, 410.0, 300.0, 910.0, 860.0);
    cover.can_stand = false;
    for changed in [
        desktop(&[source]),
        neighboring_windows(1, 300.0, 0.0),
        desktop(&[cover, source, window(2, 420.0, 350.0, 900.0, 850.0)]),
    ] {
        let mut pet = Wander::for_test(42);
        pet.place((edge.right, edge.y));
        pet.advance(0.01, 30.0, 1, &world);
        pet.jump(edge, target, &world, 1.0, 60.0);
        pet.advance(0.1, 30.0, 1, &world);
        assert_eq!(pet.pose(), Pose::Roll);
        pet.set_probability(0);
        pet.observe(true, true, 0.01);
        assert_eq!(pet.pose(), Pose::Roll);
        pet.observe(false, false, 0.01);
        pet.advance(0.4, 30.0, 1, &changed);
        assert_eq!(pet.pose(), Pose::Fall);
        assert!(pet.support.is_none());
        pet.advance(1.0, 30.0, 1, &changed);
        assert_eq!(pet.support.unwrap().window, FLOOR);
        assert_eq!(pet.pose(), Pose::Idle);
    }
}

#[test]
fn taskbar_round_trips_keep_the_existing_jump_poses() {
    for top in [200.0, 880.0] {
        let world = desktop(&[window(1, 250.0, top, 600.0, 900.0)]);
        let mut pet = grounded(&world, 300.0);
        pet.set_probability(0);
        for (edge, target) in [
            (world.edges[0], world.edges[1]),
            (world.edges[1], world.edges[0]),
        ] {
            pet.jump(edge, Footing::new(target, 300.0), &world, 1.0, 60.0);
            assert_eq!(pet.pose(), Pose::Rise);
            for _ in 0..120 {
                pet.advance(1.0 / 60.0, 30.0, 1, &world);
                assert_ne!(pet.pose(), Pose::Roll);
                if pet.support.is_some() {
                    break;
                }
            }
            assert_eq!(pet.support.unwrap().window, target.window);
        }
    }
}

#[test]
fn window_hops_need_headroom_for_the_whole_arc() {
    let world = desktop(&[
        window(1, 100.0, 40.0, 400.0, 800.0),
        window(2, 420.0, 40.0, 900.0, 850.0),
    ]);
    let edge = world.edges[1];
    let mut pet = Wander::for_test(42);
    pet.place((edge.right, edge.y));
    pet.advance(0.01, 30.0, 1, &world);
    pet.jump(
        edge,
        Footing::new(world.edges[2], world.edges[2].left),
        &world,
        1.0,
        60.0,
    );
    assert_eq!(pet.riding().unwrap().window, edge.window);
    assert_eq!(pet.pose(), Pose::Idle);
}

#[test]
fn walks_up_to_a_high_window_jumps_paces_and_gets_back_down() {
    let world = desktop(&[window(1, 550.0, 125.0, 900.0, 500.0)]);
    let mut pet = grounded(&world, 100.0);
    pet.set_probability(100);
    let (mut approached, mut rose, mut paced) = (false, false, false);
    for _ in 0..60 * 240 {
        pet.advance(1.0 / 60.0, 30.0, 1, &world);
        assert!((world.limits.top..=world.limits.bottom).contains(&pet.foot.1));
        if pet.pose() == Pose::Right && pet.foot.1 == 900.0 {
            approached = true;
        }
        if pet.pose() == Pose::Rise && pet.foot.1 < 125.0 {
            rose = true;
        }
        if pet.riding().is_some() && matches!(pet.pose(), Pose::Left | Pose::Right) {
            assert!(world.edges[1].contains(pet.foot.0));
            paced = true;
        }
        if paced && pet.support.is_some_and(|s| s.window == FLOOR) {
            assert!(approached && rose);
            return;
        }
    }
    panic!("AQUA did not complete the round trip");
}

#[test]
fn falls_onto_first_edge_rides_it_and_drops_when_covered_or_removed() {
    let low = window(2, 50.0, 600.0, 800.0, 850.0);
    let high = window(1, 80.0, 350.0, 700.0, 400.0);
    let mut pet = Wander::for_test(9);
    pet.set_probability(0);
    pet.place((200.0, 36.0));
    pet.advance(1.5, 30.0, 1, &desktop(&[high, low]));
    assert_eq!(pet.foot, (200.0, 350.0));
    let moved = window(1, 100.0, 300.0, 720.0, 350.0);
    pet.advance(0.016, 30.0, 1, &desktop(&[moved, low]));
    assert_eq!(pet.foot, (220.0, 300.0));
    let cover = window(3, 100.0, 150.0, 400.0, 410.0);
    for windows in [vec![cover, moved, low], vec![low]] {
        pet.place((220.0, 300.0));
        pet.advance(0.016, 30.0, 1, &desktop(&[moved, low]));
        assert_eq!(pet.riding().unwrap().window, 1);
        pet.advance(1.5, 30.0, 1, &desktop(&windows));
        assert_eq!(pet.foot, (220.0, 600.0));
        assert_eq!(pet.riding().unwrap().window, 2);
    }
}

#[test]
fn missing_target_and_zero_probability_in_midair_still_land() {
    let world = desktop(&[window(1, 250.0, 200.0, 600.0, 700.0)]);
    let mut pet = grounded(&world, 300.0);
    pet.jump(
        world.edges[0],
        Footing::new(world.edges[1], 320.0),
        &world,
        1.0,
        60.0,
    );
    pet.set_probability(0);
    let empty = desktop(&[]);
    for _ in 0..180 {
        pet.advance(1.0 / 60.0, 30.0, 1, &empty);
    }
    assert_eq!(pet.support.unwrap().window, FLOOR);
    let landed = pet.foot;
    pet.advance(1.9, 30.0, 1, &empty);
    assert_eq!(pet.foot, landed);
}

#[test]
fn moving_a_supported_window_to_a_negative_coordinate_monitor_keeps_local_offset() {
    let mut pet = Wander::for_test(7);
    pet.set_probability(0);
    let world = desktop(&[window(1, 100.0, 300.0, 800.0, 600.0)]);
    pet.place((250.0, 200.0));
    pet.advance(1.0, 30.0, 1, &world);
    let work = window(0, -1920.0, -200.0, 0.0, 840.0).rect;
    let body = window(0, -30.0, -72.0, 30.0, 0.0).rect;
    let moved = window(1, -1700.0, 50.0, -800.0, 600.0);
    pet.advance(0.016, 30.0, 2, &Ledges::new(work, body, &[moved]));
    assert_eq!(pet.foot, (-1550.0, 50.0));
    assert_eq!(pet.riding().unwrap().offset, 150.0);
}

#[test]
fn original_horizontal_speed_is_independent_of_timer_and_distance_setting() {
    let world = desktop(&[]);
    for scale in [1, 2, 4, 8] {
        for rate in [30, 60, 144] {
            let mut pet = grounded(&world, 100.0);
            pet.walk_to(world.edges[0], 1100.0, None);
            pet.set_distance_bias(0);
            for _ in 0..rate {
                pet.advance(1.0 / rate as f64, 30.0, scale, &world);
            }
            assert!((pet.foot.0 - 100.0 - 60.0 * scale as f64).abs() < 1e-7);
            pet.set_probability(0);
            assert_eq!(pet.pose(), Pose::Idle);
            let stopped = pet.foot;
            pet.advance(120.0, 30.0, scale, &world);
            assert_eq!(pet.foot, stopped);
        }
    }
}

#[test]
fn every_walk_setting_waits_six_point_four_seconds_before_choosing() {
    let world = desktop(&[]);
    for probability in 0..=100 {
        let mut pet = grounded(&world, 100.0);
        pet.set_probability(probability);
        for _ in 0..63 {
            pet.advance(0.1, 30.0, 1, &world);
            assert_eq!(pet.pose(), Pose::Idle, "switched early at {probability}%");
            assert_eq!(pet.foot, (100.0, 900.0));
        }
        pet.advance(0.1, 30.0, 1, &world);
        if probability == 100 {
            assert!(matches!(pet.pose(), Pose::Left | Pose::Right));
        }
    }
}

#[test]
fn distance_bias_preserves_variety_and_changes_the_average() {
    let mut means = Vec::new();
    for bias in [0, 50, 100] {
        let mut pet = Wander::for_test(42);
        pet.set_distance_bias(bias);
        let sample: Vec<_> = (0..5000).map(|_| pet.random_walk_seconds()).collect();
        assert!(sample.iter().all(|s| (1.0..=3.0).contains(s)));
        assert!(sample.iter().any(|s| *s < 1.5) && sample.iter().any(|s| *s > 2.5));
        means.push(sample.iter().sum::<f64>() / 5000.0);
    }
    assert!(means[0] < means[1] && means[1] < means[2]);
}

#[test]
fn wave_needs_a_fresh_approach_and_fifteen_seconds_between_greetings() {
    let world = desktop(&[]);
    let mut pet = grounded(&world, 100.0);
    pet.set_probability(0);
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.advance(1.99, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.advance(0.01, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Idle);
    pet.observe(false, false, 5.0);
    pet.observe(true, false, 5.0);
    assert_eq!(pet.pose(), Pose::Idle);
    pet.observe(true, false, 5.0);
    assert_eq!(pet.pose(), Pose::Idle);
    pet.observe(false, false, 0.01);
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::Wave);
    assert_eq!(pet.foot, (100.0, 900.0));
}

#[test]
fn sitting_is_random_and_requires_a_window_with_room_for_the_feet() {
    let mut sitting = 0;
    for (top, expected) in [(350.0, true), (896.0, false), (900.0, false)] {
        let world = desktop(&[window(1, 100.0, top, 800.0, 900.0)]);
        for seed in 1..=200 {
            let mut pet = Wander::for_test(seed);
            pet.set_probability(0);
            pet.place((200.0, top));
            pet.advance(0.01, 30.0, 1, &world);
            for _ in 0..64 {
                pet.advance(0.1, 30.0, 1, &world);
            }
            if pet.pose() == Pose::Perch {
                assert!(expected);
                assert_eq!(pet.riding().unwrap().window, 1);
                sitting += 1;
            }
        }
    }
    assert!(
        (25..75).contains(&sitting),
        "sat on {sitting} of 200 choices"
    );
}

#[test]
fn sitting_greets_for_two_seconds_then_sits_again_before_resuming_activity() {
    let world = desktop(&[window(1, 100.0, 350.0, 800.0, 900.0)]);
    let mut pet = Wander::for_test(42);
    pet.place((200.0, 350.0));
    pet.advance(0.01, 30.0, 1, &world);
    pet.start(Motion::Perch);
    pet.set_music_playing(true);
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::PerchWave);
    pet.advance(1.99, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::PerchWave);
    pet.advance(0.01, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Perch);
    pet.observe(false, false, 1.0);
    pet.observe(true, false, 1.0);
    assert_eq!(pet.pose(), Pose::Perch);
    for _ in 0..119 {
        pet.advance(0.1, 30.0, 1, &world);
        assert_eq!(pet.pose(), Pose::Perch);
        assert_eq!(pet.foot, (200.0, 350.0));
    }
    pet.advance(0.1, 30.0, 1, &world);
    assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));
}

#[test]
fn seated_and_waving_pets_follow_windows_and_fall_when_the_seat_disappears() {
    let high = window(1, 100.0, 350.0, 800.0, 600.0);
    let low = window(2, 50.0, 650.0, 900.0, 900.0);
    let moved = window(1, 130.0, 380.0, 830.0, 630.0);
    let cover = window(3, 100.0, 200.0, 400.0, 500.0);
    for waving in [false, true] {
        for windows in [vec![low], vec![cover, moved, low]] {
            let mut pet = Wander::for_test(42);
            pet.set_probability(0);
            pet.place((200.0, 350.0));
            pet.advance(0.01, 30.0, 1, &desktop(&[high, low]));
            pet.start(Motion::Perch);
            pet.observe(waving, false, 0.01);
            let pose = pet.pose();
            pet.advance(0.1, 30.0, 1, &desktop(&[moved, low]));
            assert_eq!(pet.pose(), pose);
            assert_eq!(pet.foot, (230.0, 380.0));
            pet.advance(0.1, 30.0, 1, &desktop(&windows));
            assert_eq!(pet.pose(), Pose::Fall);
            assert!(pet.foot.1 > 380.0);
            pet.advance(1.0, 30.0, 1, &desktop(&windows));
            assert_eq!(pet.foot, (230.0, 650.0));
            assert_eq!(pet.riding().unwrap().window, 2);
        }
    }
}

#[test]
fn sitting_releases_when_dragged_or_when_the_feet_would_leave_the_screen() {
    let world = desktop(&[window(1, 100.0, 350.0, 800.0, 900.0)]);
    let mut pet = Wander::for_test(42);
    pet.place((200.0, 350.0));
    pet.advance(0.01, 30.0, 1, &world);
    pet.start(Motion::Perch);
    pet.advance(
        0.01,
        30.0,
        1,
        &desktop(&[window(1, 100.0, 896.0, 800.0, 950.0)]),
    );
    assert_eq!(pet.pose(), Pose::Idle);
    assert_eq!(pet.foot, (200.0, 896.0));
    pet.start(Motion::Perch);
    pet.place((400.0, 250.0));
    assert_eq!(pet.pose(), Pose::Fall);
    assert!(pet.riding().is_none());
}

#[test]
fn an_inactive_pet_uses_the_window_seat_and_greets_without_standing_up() {
    let world = desktop(&[window(1, 100.0, 350.0, 800.0, 900.0)]);
    let mut pet = Wander::for_test(42);
    pet.place((200.0, 350.0));
    pet.observe(false, true, 0.01);
    pet.advance(0.1, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Perch);
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::PerchWave);
}

#[test]
fn airborne_pet_lands_before_resting_and_wakes_on_input() {
    let world = desktop(&[]);
    let mut pet = Wander::for_test(7);
    pet.place((200.0, 550.0));
    pet.observe(true, false, 0.1);
    assert_eq!(pet.pose(), Pose::Fall);
    pet.observe(false, true, 0.1);
    pet.advance(0.1, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Fall);
    pet.advance(1.5, 30.0, 1, &world);
    assert!(matches!(pet.pose(), Pose::Sit | Pose::Bored));
    let rest = pet.pose();
    pet.advance(1.8, 30.0, 1, &world);
    assert_eq!(pet.pose(), rest);
    pet.observe(false, false, 0.01);
    assert_eq!(pet.pose(), Pose::Idle);
}

#[test]
fn music_keeps_the_original_random_walks_jumps_and_daily_actions() {
    let world = desktop(&[window(1, 550.0, 125.0, 900.0, 500.0)]);
    let mut ordinary = grounded(&world, 100.0);
    let mut musical = grounded(&world, 100.0);
    musical.set_music_playing(true);

    let (mut walked, mut jumped, mut acted) = (false, false, false);
    for _ in 0..60 * 600 {
        ordinary.advance(1.0 / 60.0, 30.0, 1, &world);
        musical.advance(1.0 / 60.0, 30.0, 1, &world);
        let pose = ordinary.pose();
        if pose == Pose::Idle {
            assert!(matches!(musical.pose(), Pose::Dance | Pose::CliffSpin));
        } else {
            assert_eq!(musical.pose(), pose);
        }
        assert_eq!(musical.foot, ordinary.foot);
        walked |= matches!(pose, Pose::Left | Pose::Right);
        jumped |= pose == Pose::Rise;
        acted |= matches!(pose, Pose::Sleeve | Pose::Clap | Pose::Spin);
    }
    assert!(walked && jumped && acted);
}

#[test]
fn music_dances_stay_in_place_and_switch_after_at_least_one_minute() {
    let world = desktop(&[]);
    let mut pet = Wander::for_test(42);
    pet.set_probability(0);
    pet.place((200.0, 550.0));
    pet.set_music_playing(true);
    pet.advance(0.1, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Fall);
    pet.advance(1.0, 30.0, 1, &world);
    assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));

    let mut intervals = Vec::new();
    for _ in 0..4 {
        let dance = pet.dance.unwrap();
        let cycle = pet.pose_seconds[dance.pose as usize];
        assert!((60.0..=120.0 + cycle).contains(&dance.switch_after));
        let loops = dance.switch_after / cycle;
        assert!((loops - loops.round()).abs() < 1e-9);
        intervals.push(dance.switch_after);
        for _ in 0..2000 {
            let seconds = pet.dance.unwrap().seconds;
            pet.advance(0.1, 30.0, 1, &world);
            assert_eq!(pet.foot, (200.0, 900.0));
            if pet.dance.unwrap().pose != dance.pose {
                assert!(seconds + 0.1 + 1e-9 >= dance.switch_after);
                break;
            }
        }
        assert_ne!(pet.dance.unwrap().pose, dance.pose);
    }
    assert!(intervals.iter().any(|&seconds| seconds != intervals[0]));
    pet.observe(false, true, 0.1);
    pet.advance(0.1, 30.0, 1, &world);
    assert!(matches!(pet.pose(), Pose::Sit | Pose::Bored));
    pet.observe(false, false, 0.1);
    assert!(matches!(pet.pose(), Pose::Dance | Pose::CliffSpin));
    pet.set_music_playing(false);
    assert_eq!(pet.pose(), Pose::Idle);
}

#[test]
fn a_greeting_pauses_the_current_dance_and_music_can_stop_during_the_wave() {
    let world = desktop(&[]);
    let mut pet = grounded(&world, 100.0);
    pet.set_music_playing(true);
    pet.advance(0.9, 30.0, 1, &world);
    let pose = pet.pose();
    let seconds = pet.animation_seconds();
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.advance(1.99, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.advance(0.01, 30.0, 1, &world);
    assert_eq!(pet.pose(), pose);
    assert_eq!(pet.animation_seconds(), seconds);

    pet.observe(false, false, 15.0);
    pet.observe(true, false, 0.01);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.set_music_playing(false);
    pet.advance(1.99, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Wave);
    pet.advance(0.01, 30.0, 1, &world);
    assert_eq!(pet.pose(), Pose::Idle);
}

#[test]
fn daily_actions_last_six_seconds_and_finish_on_a_complete_loop() {
    let world = desktop(&[]);
    for (pose, seconds) in [(Pose::Sleeve, 6.4), (Pose::Clap, 6.0), (Pose::Spin, 6.0)] {
        for rate in [10, 60, 144] {
            let mut pet = grounded(&world, 100.0);
            pet.set_probability(100);
            pet.start(Motion::Action(pose));
            let frames = (seconds * rate as f64).ceil() as usize;
            for _ in 1..frames {
                pet.advance(1.0 / rate as f64, 30.0, 1, &world);
                assert_eq!(pet.pose(), pose, "{pose:?} ended early at {rate} updates/s");
                assert_eq!(pet.foot, (100.0, 900.0));
            }
            pet.advance(1.0 / rate as f64, 30.0, 1, &world);
            assert_eq!(pet.pose(), Pose::Idle);
        }
    }
}

#[test]
fn spontaneous_actions_finish_and_do_not_repeat_even_at_zero_walk_chance() {
    let world = desktop(&[]);
    let mut pet = grounded(&world, 100.0);
    pet.set_probability(0);
    let mut last = None;
    let mut previous = Pose::Idle;
    let mut seen = Vec::new();
    for _ in 0..60 * 240 {
        pet.advance(1.0 / 60.0, 30.0, 1, &world);
        let pose = pet.pose();
        if previous == Pose::Idle && pose != Pose::Idle {
            assert!(matches!(pose, Pose::Sleeve | Pose::Clap | Pose::Spin));
            assert_ne!(Some(pose), last);
            last = Some(pose);
            if !seen.contains(&pose) {
                seen.push(pose);
            }
        }
        previous = pose;
    }
    assert_eq!(seen.len(), 3);
    assert_eq!(pet.foot, (100.0, 900.0));
}
