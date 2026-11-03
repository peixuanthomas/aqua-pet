use super::*;
use crate::{ledges::Window, render::Bounds};

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
    let mut pet = Wander::new(42);
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
    let mut pet = Wander::new(9);
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
        let mut pet = Wander::new(42);
        pet.set_distance_bias(bias);
        let sample: Vec<_> = (0..5000).map(|_| pet.random_walk_seconds()).collect();
        assert!(sample.iter().all(|s| (1.0..=3.0).contains(s)));
        assert!(sample.iter().any(|s| *s < 1.5) && sample.iter().any(|s| *s > 2.5));
        means.push(sample.iter().sum::<f64>() / 5000.0);
    }
    assert!(means[0] < means[1] && means[1] < means[2]);
}
