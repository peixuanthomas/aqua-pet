use crate::render::Bounds;

// obj_ch5_DWCR_documents_Step_0:490: 240 world pixels / 60 game ticks.
// Dark-world sprites are drawn at 2x, so this is 2 source pixels per tick.
const SOURCE_PIXELS_PER_TICK: f64 = 240.0 / 60.0 / 2.0;
pub const DEFAULT_WALK_CHANCE: u8 = 25;
pub const DEFAULT_DISTANCE_BIAS: u8 = 50;
const IDLE_SECONDS: f64 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::Up, Self::Down, Self::Left, Self::Right];

    pub fn clip_id(self) -> &'static str {
        match self {
            Self::Up => "walk-up",
            Self::Down => "walk-down",
            Self::Left => "walk-left",
            Self::Right => "walk-right",
        }
    }

    fn vector(self) -> (f64, f64) {
        match self {
            Self::Up => (0.0, -1.0),
            Self::Down => (0.0, 1.0),
            Self::Left => (-1.0, 0.0),
            Self::Right => (1.0, 0.0),
        }
    }

    fn room(self, position: (f64, f64), limits: Bounds) -> f64 {
        match self {
            Self::Up => position.1 - limits.top,
            Self::Down => limits.bottom - position.1,
            Self::Left => position.0 - limits.left,
            Self::Right => limits.right - position.0,
        }
        .max(0.0)
    }
}

/// Idle timing and direction choices belong to the desktop pet. Only the walk
/// speed and animation clips come from the game.
pub struct Wander {
    rng: u64,
    direction: Option<Direction>,
    remaining: f64,
    animation_seconds: f64,
    fraction: (f64, f64),
    probability: u8,
    distance_bias: u8,
}

impl Wander {
    pub fn new(seed: u64) -> Self {
        let mut wander = Self {
            rng: seed.max(1),
            direction: None,
            remaining: 0.0,
            animation_seconds: 0.0,
            fraction: (0.0, 0.0),
            probability: DEFAULT_WALK_CHANCE,
            distance_bias: DEFAULT_DISTANCE_BIAS,
        };
        wander.reset();
        wander
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let bits = self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (bits >> 11) as f64 / ((1u64 << 53) as f64)
    }

    fn idle(&mut self) {
        self.direction = None;
        self.remaining = IDLE_SECONDS;
        self.animation_seconds = 0.0;
    }

    pub fn reset(&mut self) {
        self.idle();
        self.fraction = (0.0, 0.0);
    }

    pub fn direction(&self) -> Option<Direction> {
        self.direction
    }

    pub fn probability(&self) -> u8 {
        self.probability
    }

    pub fn set_probability(&mut self, probability: u8) {
        self.probability = probability.min(100);
        if self.probability == 0 {
            self.reset();
        }
    }

    pub fn animation_seconds(&self) -> f64 {
        self.animation_seconds
    }

    pub fn distance_bias(&self) -> u8 {
        self.distance_bias
    }

    pub fn set_distance_bias(&mut self, bias: u8) {
        // Only future routes change; don't stop or resize a walk in progress.
        self.distance_bias = bias.min(100);
    }

    fn random_walk_seconds(&mut self) -> f64 {
        let sample = self.random();
        let bias = self.distance_bias as f64 / 100.0;
        // Keep the whole 1..3 second range random at either endpoint. The
        // midpoint is uniform, preserving the original distance distribution.
        let weighted = if bias < 0.5 {
            sample.powf(1.0 + (0.5 - bias) * 4.0)
        } else {
            1.0 - (1.0 - sample).powf(1.0 + (bias - 0.5) * 4.0)
        };
        1.0 + weighted * 2.0
    }

    pub fn advance(
        &mut self,
        seconds: f64,
        game_fps: f64,
        scale: u32,
        anchor: (i32, i32),
        limits: Bounds,
    ) -> (i32, i32) {
        // A resume from sleep must not spend the missing time walking.
        if !seconds.is_finite() || seconds <= 0.0 || seconds >= 2.0 {
            return anchor;
        }
        let speed = SOURCE_PIXELS_PER_TICK * game_fps * scale.max(1) as f64;
        let mut position = (
            (anchor.0 as f64 + self.fraction.0).clamp(limits.left, limits.right),
            (anchor.1 as f64 + self.fraction.1).clamp(limits.top, limits.bottom),
        );
        let mut elapsed = seconds;
        while elapsed > 1e-9 {
            if self.remaining <= 1e-9 {
                if self.direction.is_some() || self.random() * 100.0 >= self.probability as f64 {
                    self.idle();
                } else {
                    // Don't pick an edge-facing direction or a path shorter
                    // than half a second; otherwise the pet just flickers.
                    let candidates: Vec<_> = Direction::ALL
                        .into_iter()
                        .filter(|d| d.room(position, limits) >= speed * 0.5)
                        .collect();
                    if candidates.is_empty() {
                        self.idle();
                    } else {
                        let direction =
                            candidates[(self.random() * candidates.len() as f64) as usize];
                        self.direction = Some(direction);
                        self.remaining = self
                            .random_walk_seconds()
                            .min(direction.room(position, limits) / speed);
                        self.animation_seconds = 0.0;
                    }
                }
            }
            let mut step = elapsed.min(self.remaining);
            if let Some(direction) = self.direction {
                let until_edge = direction.room(position, limits) / speed;
                step = step.min(until_edge);
                let (dx, dy) = direction.vector();
                position.0 = (position.0 + dx * speed * step).clamp(limits.left, limits.right);
                position.1 = (position.1 + dy * speed * step).clamp(limits.top, limits.bottom);
                if step >= until_edge - 1e-9 {
                    self.remaining = step;
                }
            }
            elapsed -= step;
            self.remaining -= step;
            self.animation_seconds += step;
            if self.direction.is_some() && self.remaining <= 1e-9 {
                self.idle();
            }
        }
        let rounded = (position.0.round() as i32, position.1.round() as i32);
        // Keep fractions across timer callbacks, including at 1x. Rounding each
        // individual movement would change the speed with the callback rate.
        self.fraction = (position.0 - rounded.0 as f64, position.1 - rounded.1 as f64);
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn limits() -> Bounds {
        Bounds {
            left: -10000.0,
            top: -10000.0,
            right: 10000.0,
            bottom: 10000.0,
        }
    }

    fn walking(direction: Direction) -> Wander {
        let mut wander = Wander::new(7);
        wander.direction = Some(direction);
        wander.remaining = 10.0;
        wander
    }

    #[test]
    fn starts_idle_and_reset_cancels_a_walk() {
        let mut wander = Wander::new(7);
        assert_eq!(wander.direction(), None);
        assert_eq!(wander.advance(1.0, 30.0, 4, (50, 60), limits()), (50, 60));
        assert_eq!(wander.animation_seconds(), 1.0);
        wander = walking(Direction::Left);
        wander.reset();
        assert_eq!(wander.direction(), None);
        assert_eq!(wander.remaining, IDLE_SECONDS);
        assert_eq!(wander.animation_seconds(), 0.0);
    }

    #[test]
    fn original_speed_in_all_directions_scales_without_timer_rounding_drift() {
        for direction in Direction::ALL {
            for scale in [1, 2, 4, 8] {
                for callbacks in [30, 60, 144] {
                    let mut wander = walking(direction);
                    let mut anchor = (0, 0);
                    for _ in 0..callbacks {
                        anchor =
                            wander.advance(1.0 / callbacks as f64, 30.0, scale, anchor, limits());
                    }
                    let (dx, dy) = direction.vector();
                    let distance = 60.0 * scale as f64;
                    assert_eq!(anchor, ((dx * distance) as i32, (dy * distance) as i32));
                }
            }
        }
    }

    #[test]
    fn stops_at_edge_and_stays_at_destination() {
        let mut wander = walking(Direction::Right);
        let limits = Bounds {
            left: -500.0,
            top: -500.0,
            right: -100.0,
            bottom: -100.0,
        };
        let anchor = wander.advance(1.0, 30.0, 4, (-110, -200), limits);
        assert_eq!(anchor, (-100, -200));
        assert_eq!(wander.direction(), None);
        assert_eq!(wander.advance(1.0, 30.0, 4, anchor, limits), anchor);
    }

    #[test]
    fn random_walks_cover_four_directions_and_never_leave_work_area() {
        let limits = Bounds {
            left: -1800.0,
            top: -100.0,
            right: -200.0,
            bottom: 800.0,
        };
        let mut wander = Wander::new(42);
        let mut anchor = (-200, 800);
        let mut seen = [false; 4];
        for _ in 0..36000 {
            anchor = wander.advance(1.0 / 30.0, 30.0, 4, anchor, limits);
            if let Some(direction) = wander.direction() {
                seen[direction as usize] = true;
            }
            assert!((limits.left..=limits.right).contains(&(anchor.0 as f64)));
            assert!((limits.top..=limits.bottom).contains(&(anchor.1 as f64)));
        }
        assert!(seen.into_iter().all(|seen| seen));
    }

    #[test]
    fn blocked_space_and_sleep_do_not_move_or_spin() {
        let mut wander = Wander::new(7);
        wander.remaining = 0.0;
        let limits = Bounds {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
        assert_eq!(wander.advance(1.0, 30.0, 4, (0, 0), limits), (0, 0));
        assert_eq!(wander.direction(), None);
        let mut wander = walking(Direction::Down);
        assert_eq!(wander.advance(120.0, 30.0, 4, (0, 0), limits), (0, 0));
        assert_eq!(wander.animation_seconds(), 0.0);
    }

    #[test]
    fn existing_clips_match_the_documented_walk_speed_and_timing() {
        let catalog = Catalog::load().unwrap();
        for direction in Direction::ALL {
            let clip = catalog
                .clips
                .iter()
                .find(|c| c.id == direction.clip_id())
                .unwrap();
            assert!(clip.looped);
            assert!(
                clip.steps
                    .iter()
                    .all(|s| s.ticks == 4.0 && s.root_x == 0.0 && s.root_y == 0.0)
            );
        }
        let reference = catalog
            .clips
            .iter()
            .find(|c| c.id == "walk-right-documents")
            .unwrap();
        assert_eq!(
            reference.steps.last().unwrap().root_x / reference.duration(),
            SOURCE_PIXELS_PER_TICK
        );
        assert_eq!(
            catalog.clips[catalog.idle_index()].duration() / catalog.game_fps,
            IDLE_SECONDS
        );
    }

    #[test]
    fn slider_zero_stops_immediately_and_maximum_walks_after_each_idle_cycle() {
        let mut wander = walking(Direction::Right);
        wander.set_probability(0);
        assert_eq!(wander.direction(), None);
        for _ in 0..600 {
            assert_eq!(wander.advance(1.0, 30.0, 4, (0, 0), limits()), (0, 0));
            assert_eq!(wander.direction(), None);
        }
        wander.set_probability(100);
        wander.reset();
        wander.advance(1.0, 30.0, 4, (0, 0), limits());
        assert_eq!(wander.direction(), None);
        assert_ne!(wander.advance(1.0, 30.0, 4, (0, 0), limits()), (0, 0));
        assert!(wander.direction().is_some());
    }

    #[test]
    fn probability_changes_frequency_without_changing_walk_speed() {
        let mut walking_time = Vec::new();
        for probability in [0, DEFAULT_WALK_CHANCE, 100] {
            let mut wander = Wander::new(42);
            wander.set_probability(probability);
            let mut anchor = (0, 0);
            let mut count = 0;
            for _ in 0..36000 {
                anchor = wander.advance(1.0 / 30.0, 30.0, 4, anchor, limits());
                count += usize::from(wander.direction().is_some());
            }
            walking_time.push(count);
        }
        assert_eq!(walking_time[0], 0);
        assert!(walking_time[1] > 0);
        assert!(walking_time[2] > walking_time[1] * 2);
    }

    #[test]
    fn distance_bias_changes_average_but_all_routes_remain_random() {
        let mut averages = Vec::new();
        for bias in [0, 25, 50, 75, 100] {
            let mut wander = Wander::new(42);
            wander.set_distance_bias(bias);
            let samples: Vec<_> = (0..10000).map(|_| wander.random_walk_seconds()).collect();
            assert!(samples.iter().all(|s| (1.0..=3.0).contains(s)));
            // Even at either extreme there are both short and long routes.
            assert!(samples.iter().any(|s| *s < 1.5));
            assert!(samples.iter().any(|s| *s > 2.5));
            averages.push(samples.iter().sum::<f64>() / samples.len() as f64);
        }
        assert!(averages.windows(2).all(|pair| pair[0] < pair[1]));
        assert!((averages[0] - 1.5).abs() < 0.03);
        assert!((averages[2] - 2.0).abs() < 0.03);
        assert!((averages[4] - 2.5).abs() < 0.03);
    }

    #[test]
    fn changing_distance_bias_keeps_current_walk_and_original_speed() {
        let mut wander = walking(Direction::Right);
        let mut anchor = (0, 0);
        for bias in [0, 50, 100] {
            let remaining = wander.remaining;
            wander.set_distance_bias(bias);
            assert_eq!(wander.remaining, remaining);
            assert_eq!(wander.probability(), DEFAULT_WALK_CHANCE);
            let next = wander.advance(0.5, 30.0, 4, anchor, limits());
            assert_eq!(next.0 - anchor.0, 120);
            anchor = next;
        }
    }
}
