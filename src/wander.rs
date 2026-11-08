use crate::{
    ledges::{FLOOR, Ledge, Ledges},
    poses::{PERCH_DEPTH, Pose},
};

// The original walk moves 240 world pixels in 60 game ticks, at 2x sprite scale.
const SOURCE_PIXELS_PER_TICK: f64 = 2.0;
pub const DEFAULT_WALK_CHANCE: u8 = 25;
pub const DEFAULT_DISTANCE_BIAS: u8 = 50;
const IDLE_SECONDS: f64 = 6.4;
const MIN_DAILY_SECONDS: f64 = 6.0;
const MOUSE_WAVE_SECONDS: f64 = 2.0;
const PERCH_SECONDS: f64 = 12.0;
const MIN_DANCE_SECONDS: f64 = 60.0;
const MAX_DANCE_SECONDS: f64 = 120.0;
const MAX_HOP_X: f64 = 50.0;
const MAX_HOP_Y: f64 = 35.0;

#[derive(Clone, Copy, Debug)]
pub struct Footing {
    pub window: usize,
    pub offset: f64,
}

impl Footing {
    fn new(edge: Ledge, x: f64) -> Self {
        Self {
            window: edge.window,
            offset: x - edge.origin_x,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Dance {
    pose: Pose,
    seconds: f64,
    switch_after: f64,
}

#[derive(Clone, Copy, Debug)]
enum Motion {
    Idle,
    Action(Pose),
    Rest(Pose),
    Perch,
    Wave {
        seated: bool,
    },
    Walk {
        right: bool,
        end: f64,
        jump_to: Option<Footing>,
    },
    Air {
        vx: f64,
        vy: f64,
        skip: Option<usize>,
        roll: bool,
    },
}

pub struct Wander {
    rng: u64,
    pub foot: (f64, f64),
    pub support: Option<Footing>,
    motion: Motion,
    motion_seconds: f64,
    probability: u8,
    distance_bias: u8,
    pose_seconds: [f64; Pose::ALL.len()],
    last_daily: Option<Pose>,
    mouse_near: bool,
    wave_cooldown: f64,
    computer_idle: bool,
    dance: Option<Dance>,
    dance_rng: u64,
}

impl Wander {
    pub fn new(seed: u64, mut pose_seconds: [f64; Pose::ALL.len()]) -> Self {
        // Repeat whole loops so short daily actions stay visible without slowing their animation.
        for pose in [Pose::Sleeve, Pose::Clap, Pose::Spin] {
            let seconds = &mut pose_seconds[pose as usize];
            *seconds *= (MIN_DAILY_SECONDS / *seconds).ceil();
        }
        Self {
            rng: seed.max(1),
            foot: (0.0, 0.0),
            support: None,
            motion: Motion::Idle,
            motion_seconds: 0.0,
            probability: DEFAULT_WALK_CHANCE,
            distance_bias: DEFAULT_DISTANCE_BIAS,
            pose_seconds,
            last_daily: None,
            mouse_near: false,
            wave_cooldown: 0.0,
            computer_idle: false,
            dance: None,
            dance_rng: (!seed).max(1),
        }
    }

    fn random(&mut self) -> f64 {
        Self::random_sample(&mut self.rng)
    }

    fn random_sample(rng: &mut u64) -> f64 {
        *rng ^= *rng >> 12;
        *rng ^= *rng << 25;
        *rng ^= *rng >> 27;
        let bits = rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (bits >> 11) as f64 / ((1u64 << 53) as f64)
    }

    fn start(&mut self, motion: Motion) {
        self.motion = motion;
        self.motion_seconds = 0.0;
    }

    pub fn place(&mut self, foot: (f64, f64)) {
        self.foot = foot;
        self.support = None;
        self.start(Motion::Air {
            vx: 0.0,
            vy: 0.0,
            skip: None,
            roll: false,
        });
    }

    pub fn pose(&self) -> Pose {
        match self.motion {
            Motion::Idle => self.dance.map_or(Pose::Idle, |dance| dance.pose),
            Motion::Action(pose) | Motion::Rest(pose) => pose,
            Motion::Perch => Pose::Perch,
            Motion::Wave { seated: true } => Pose::PerchWave,
            Motion::Wave { seated: false } => Pose::Wave,
            Motion::Walk { right: true, .. } => Pose::Right,
            Motion::Walk { right: false, .. } => Pose::Left,
            Motion::Air { roll: true, .. }
                if self.motion_seconds + 1e-9 >= self.pose_seconds[Pose::Rise as usize]
                    && self.motion_seconds + 1e-9
                        < self.pose_seconds[Pose::Rise as usize]
                            + self.pose_seconds[Pose::Roll as usize] =>
            {
                Pose::Roll
            }
            Motion::Air { vy, .. } if vy < 0.0 => Pose::Rise,
            Motion::Air { .. } => Pose::Fall,
        }
    }

    pub fn animation_seconds(&self) -> f64 {
        match (self.motion, self.dance) {
            (Motion::Idle, Some(dance)) => dance.seconds,
            (Motion::Air { roll: true, .. }, _) if self.pose() == Pose::Roll => {
                (self.motion_seconds - self.pose_seconds[Pose::Rise as usize]).max(0.0)
            }
            _ => self.motion_seconds,
        }
    }

    pub fn probability(&self) -> u8 {
        self.probability
    }

    pub fn observe(&mut self, near: bool, idle: bool, elapsed: f64) {
        self.wave_cooldown = (self.wave_cooldown - elapsed).max(0.0);
        self.computer_idle = idle;
        let seated = self.pose() == Pose::Perch;
        if !idle && matches!(self.motion, Motion::Rest(_)) {
            self.start(Motion::Idle);
        }
        if near && !self.mouse_near && !idle && self.wave_cooldown == 0.0 && self.support.is_some()
        {
            self.start(Motion::Wave { seated });
            self.wave_cooldown = 15.0;
        }
        self.mouse_near = near;
    }

    pub fn set_music_playing(&mut self, playing: bool) {
        if !playing {
            self.dance = None;
        } else if self.dance.is_none() {
            let pose = if Self::random_sample(&mut self.dance_rng) < 0.5 {
                Pose::Dance
            } else {
                Pose::CliffSpin
            };
            self.dance = Some(self.new_dance(pose));
        }
    }

    fn new_dance(&mut self, pose: Pose) -> Dance {
        // Dance choices must not consume the random numbers used for movement.
        let seconds = MIN_DANCE_SECONDS
            + Self::random_sample(&mut self.dance_rng) * (MAX_DANCE_SECONDS - MIN_DANCE_SECONDS);
        let cycle = self.pose_seconds[pose as usize];
        // Finish the last loop before changing steps.
        Dance {
            pose,
            seconds: 0.0,
            switch_after: (seconds / cycle).ceil() * cycle,
        }
    }

    pub fn riding(&self) -> Option<Footing> {
        self.support.filter(|s| s.window != FLOOR)
    }

    fn can_perch(&self, world: &Ledges, scale: f64) -> bool {
        self.riding().is_some() && self.foot.1 + PERCH_DEPTH * scale <= world.limits.bottom
    }

    pub fn set_probability(&mut self, probability: u8) {
        self.probability = probability.min(100);
        if self.probability == 0 && matches!(self.motion, Motion::Walk { .. }) {
            self.start(Motion::Idle);
        }
    }

    pub fn distance_bias(&self) -> u8 {
        self.distance_bias
    }

    pub fn set_distance_bias(&mut self, bias: u8) {
        self.distance_bias = bias.min(100);
    }

    fn random_walk_seconds(&mut self) -> f64 {
        let sample = self.random();
        let bias = self.distance_bias as f64 / 100.0;
        let weighted = if bias < 0.5 {
            sample.powf(1.0 + (0.5 - bias) * 4.0)
        } else {
            1.0 - (1.0 - sample).powf(1.0 + (bias - 0.5) * 4.0)
        };
        1.0 + weighted * 2.0
    }

    fn walk_to(&mut self, edge: Ledge, x: f64, jump_to: Option<Footing>) {
        self.start(Motion::Walk {
            right: x >= self.foot.0,
            end: x - edge.origin_x,
            jump_to,
        });
    }

    fn choose(&mut self, edge: Ledge, world: &Ledges, speed: f64, scale: f64) {
        self.start(Motion::Idle);
        if self.random() * 100.0 >= self.probability as f64 {
            if self.can_perch(world, scale) && self.random() < 0.25 {
                self.start(Motion::Perch);
                return;
            }
            if self.random() < 0.15 {
                let daily = [Pose::Sleeve, Pose::Clap, Pose::Spin];
                let choices: Vec<_> = daily
                    .into_iter()
                    .filter(|p| Some(*p) != self.last_daily)
                    .collect();
                let pose = choices[(self.random() * choices.len() as f64) as usize];
                self.last_daily = Some(pose);
                self.start(Motion::Action(pose));
            }
            return;
        }
        let x = self.foot.0;
        if self.random() < 0.5 {
            let targets: Vec<_> = world
                .edges
                .iter()
                .filter(|other| {
                    other.window != edge.window
                        && other.left <= edge.right + speed
                        && other.right >= edge.left - speed
                })
                .filter_map(|&target| {
                    let mut landing = x.clamp(target.left, target.right);
                    if target.y > edge.y {
                        // Prefer jumping past an end of the current ledge when the lower one extends farther.
                        let exits = [edge.left - speed * 0.4, edge.right + speed * 0.4];
                        if let Some(exit) = exits
                            .into_iter()
                            .filter(|x| target.contains(*x))
                            .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
                        {
                            landing = exit;
                        }
                    }
                    let takeoff = landing.clamp(edge.left, edge.right);
                    if edge.window != FLOOR
                        && target.window != FLOOR
                        && self
                            .hop_velocity((takeoff, edge.y), (landing, target.y), scale, world)
                            .is_none()
                    {
                        return None;
                    }
                    Some((takeoff, Footing::new(target, landing)))
                })
                .collect();
            if !targets.is_empty() {
                let (takeoff, target) = targets[(self.random() * targets.len() as f64) as usize];
                if (takeoff - x).abs() < 0.5 {
                    self.jump(edge, target, world, scale, speed);
                } else {
                    self.walk_to(edge, takeoff, Some(target));
                }
                return;
            }
        }
        let left = x - edge.left;
        let right = edge.right - x;
        if left.max(right) < 1.0 {
            return;
        }
        let direction = if right < 1.0 || (left >= 1.0 && self.random() < 0.5) {
            -1.0
        } else {
            1.0
        };
        let end = (x + direction * speed * self.random_walk_seconds()).clamp(edge.left, edge.right);
        self.walk_to(edge, end, None);
    }

    fn hop_velocity(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        scale: f64,
        world: &Ledges,
    ) -> Option<(f64, f64)> {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        if dx.abs() > MAX_HOP_X * scale + 1e-7 || dy.abs() > MAX_HOP_Y * scale + 1e-7 {
            return None;
        }
        // Reach the next window as the one-shot roll ends, without slowing the original frames.
        let duration =
            self.pose_seconds[Pose::Rise as usize] + self.pose_seconds[Pose::Roll as usize];
        let gravity = 900.0 * scale;
        let vy = dy / duration - 0.5 * gravity * duration;
        let peak = from.1 - vy * vy / (2.0 * gravity);
        (peak >= world.limits.top).then_some((dx / duration, vy))
    }

    fn jump(&mut self, edge: Ledge, target: Footing, world: &Ledges, scale: f64, speed: f64) {
        let Some(landing) = world.at_offset(target.window, target.offset) else {
            return;
        };
        let dx = landing.origin_x + target.offset - self.foot.0;
        if dx.abs() > speed + 0.01 {
            return;
        }
        let roll = edge.window != FLOOR && landing.window != FLOOR;
        let (vx, vy) = if roll {
            let Some(velocity) = self.hop_velocity(
                self.foot,
                (landing.origin_x + target.offset, landing.y),
                scale,
                world,
            ) else {
                return;
            };
            velocity
        } else {
            let gravity = 900.0 * scale;
            let peak = (edge.y.min(landing.y) - 12.0 * scale).max(world.limits.top);
            let vy = -(2.0 * gravity * (edge.y - peak)).sqrt();
            let duration = -vy / gravity + (2.0 * (landing.y - peak) / gravity).sqrt();
            if duration <= 0.0 {
                return;
            }
            (dx / duration, vy)
        };
        self.support = None;
        self.start(Motion::Air {
            vx,
            vy,
            skip: (landing.y > edge.y).then_some(edge.window),
            roll,
        });
    }

    fn fly(&mut self, dt: f64, scale: f64, world: &Ledges, vx: f64, vy: f64, skip: Option<usize>) {
        let (x, y) = self.foot;
        let gravity = 900.0 * scale;
        let next_vy = vy + gravity * dt;
        let x_at = |t: f64| (x + vx * t).clamp(world.limits.left, world.limits.right);
        if next_vy >= 0.0 {
            // Solve for the descending crossing, including a ledge touched exactly at the apex.
            let hit = world
                .edges
                .iter()
                .filter(|e| Some(e.window) != skip)
                .filter_map(|e| {
                    let d = vy * vy + 2.0 * gravity * (e.y - y);
                    if d < -1e-6 {
                        return None;
                    }
                    let t = (-vy + d.max(0.0).sqrt()) / gravity;
                    let at = x_at(t);
                    (t >= -1e-8 && t <= dt + 1e-8 && at >= e.left - 1e-7 && at <= e.right + 1e-7)
                        .then(|| (t, *e, at.clamp(e.left, e.right)))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, edge, at)) = hit {
                self.foot = (at, edge.y);
                self.support = Some(Footing::new(edge, at));
                self.start(Motion::Idle);
                return;
            }
        }
        self.foot = (
            x_at(dt),
            (y + vy * dt + 0.5 * gravity * dt * dt).max(world.limits.top),
        );
        if let Motion::Air { vy, .. } = &mut self.motion {
            *vy = next_vy;
        }
    }

    pub fn advance(&mut self, seconds: f64, game_fps: f64, scale: u32, world: &Ledges) {
        if !(0.0..2.0).contains(&seconds) {
            return;
        }
        let support = self
            .support
            .and_then(|s| world.at_offset(s.window, s.offset).map(|e| (e, s.offset)));
        if let Some((edge, offset)) = support {
            self.foot = (edge.origin_x + offset, edge.y);
        } else if !matches!(self.motion, Motion::Air { .. }) {
            self.place(self.foot);
        }
        self.foot.0 = self.foot.0.clamp(world.limits.left, world.limits.right);
        self.foot.1 = self.foot.1.clamp(world.limits.top, world.limits.bottom);
        let scale = scale.max(1) as f64;
        let can_perch = self.can_perch(world, scale);
        if !can_perch && matches!(self.pose(), Pose::Perch | Pose::PerchWave) {
            self.start(Motion::Idle);
        }
        let speed = SOURCE_PIXELS_PER_TICK * game_fps * scale;
        let mut remaining = seconds;
        while remaining > 1e-9 {
            let dt = remaining.min(1.0 / 60.0);
            remaining -= dt;
            self.motion_seconds += dt;
            if let Motion::Air { vx, vy, skip, .. } = self.motion {
                self.fly(dt, scale, world, vx, vy, skip);
                continue;
            }
            if let Motion::Wave { seated } = self.motion {
                if self.motion_seconds + 1e-9 >= MOUSE_WAVE_SECONDS {
                    self.start(if seated { Motion::Perch } else { Motion::Idle });
                }
                continue;
            }
            let standing = self.support.expect("ground motion requires a ledge");
            let edge = world
                .at_offset(standing.window, standing.offset)
                .expect("standing on visible ledge");
            if self.computer_idle && !matches!(self.motion, Motion::Rest(_) | Motion::Perch) {
                let pose = if self.can_perch(world, scale) {
                    Pose::Perch
                } else if self.random() < 0.5 {
                    Pose::Sit
                } else {
                    Pose::Bored
                };
                self.start(Motion::Rest(pose));
            }
            match self.motion {
                Motion::Perch if self.motion_seconds + 1e-9 >= PERCH_SECONDS => {
                    self.start(Motion::Idle)
                }
                Motion::Action(pose)
                    if self.motion_seconds + 1e-9 >= self.pose_seconds[pose as usize] =>
                {
                    self.start(Motion::Idle)
                }
                Motion::Idle => {
                    if let Some(dance) = &mut self.dance {
                        dance.seconds += dt;
                        if dance.seconds + 1e-9 >= dance.switch_after {
                            let next = if dance.pose == Pose::Dance {
                                Pose::CliffSpin
                            } else {
                                Pose::Dance
                            };
                            self.dance = Some(self.new_dance(next));
                        }
                    }
                    if self.motion_seconds + 1e-9 >= IDLE_SECONDS {
                        self.choose(edge, world, speed, scale);
                    }
                }
                Motion::Walk { end, jump_to, .. } => {
                    let end = (edge.origin_x + end).clamp(edge.left, edge.right);
                    let dx = end - self.foot.0;
                    self.foot.0 += dx.clamp(-speed * dt, speed * dt);
                    self.support = Some(Footing::new(edge, self.foot.0));
                    if dx.abs() <= speed * dt {
                        self.start(Motion::Idle);
                        if let Some(target) = jump_to {
                            self.jump(edge, target, world, scale, speed);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
