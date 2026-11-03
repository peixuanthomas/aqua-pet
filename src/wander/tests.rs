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
    pet.advance(pet.pose_seconds[Pose::Wave as usize], 30.0, 1, &world);
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
