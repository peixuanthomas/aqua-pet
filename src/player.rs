use crate::catalog::Clip;
use std::collections::HashSet;

pub struct Player {
    pub selected: usize,
    pub tick: f64,
    pub playing: bool,
    pub looping: bool,
    pub rate: f64,
    pub waiting: Option<usize>,
    pub finished: bool,
    released: HashSet<usize>,
}

impl Player {
    pub fn new(selected: usize, clip: &Clip) -> Self {
        Self {
            selected,
            tick: 0.0,
            playing: true,
            looping: clip.looped,
            rate: 1.0,
            waiting: None,
            finished: false,
            released: HashSet::new(),
        }
    }
    pub fn select(&mut self, selected: usize, clip: &Clip) {
        *self = Self::new(selected, clip);
    }
    pub fn restart(&mut self) {
        self.tick = 0.0;
        self.finished = false;
        self.waiting = None;
        self.released.clear();
        self.playing = true;
    }
    pub fn toggle(&mut self) {
        if self.waiting.take().is_some_and(|i| {
            self.released.insert(i);
            true
        }) {
            self.playing = true;
        } else if self.finished {
            self.restart();
        } else {
            self.playing = !self.playing;
        }
    }
    pub fn seek(&mut self, clip: &Clip, tick: f64) {
        self.tick = tick.clamp(0.0, clip.duration());
        self.playing = false;
        self.waiting = None;
        self.finished = self.tick >= clip.duration();
        self.released.clear();
        for (i, s) in clip.steps.iter().enumerate() {
            if s.wait && clip.step_start(i) + s.ticks <= self.tick {
                self.released.insert(i);
            }
        }
    }
    pub fn step(&mut self, clip: &Clip, forward: bool) {
        let current = self.index(clip);
        let next = if forward {
            (current + 1).min(clip.steps.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.seek(clip, clip.step_start(next));
    }
    pub fn index(&self, clip: &Clip) -> usize {
        self.waiting.unwrap_or_else(|| clip.step_at(self.tick))
    }
    pub fn root(&self, clip: &Clip) -> (f64, f64) {
        if self.finished {
            (0.0, 0.0)
        } else {
            let step = &clip.steps[self.index(clip)];
            (step.root_x, step.root_y)
        }
    }
    pub fn advance(&mut self, seconds: f64, fps: f64, clip: &Clip) {
        if !self.playing || self.waiting.is_some() || self.finished || seconds <= 0.0 {
            return;
        }
        let mut remaining = seconds * fps * self.rate;
        while remaining > 1e-9 {
            let mut boundary = clip.duration();
            let mut wait_index = None;
            let mut end = 0.0;
            for (i, step) in clip.steps.iter().enumerate() {
                end += step.ticks;
                if step.wait && !self.released.contains(&i) && end >= self.tick - 1e-9 {
                    boundary = end;
                    wait_index = Some(i);
                    break;
                }
            }
            let distance = (boundary - self.tick).max(0.0);
            if remaining + 1e-9 < distance {
                self.tick += remaining;
                break;
            }
            self.tick = boundary;
            remaining -= distance;
            if let Some(i) = wait_index {
                self.waiting = Some(i);
                self.playing = false;
                break;
            }
            if self.looping {
                self.tick = 0.0;
                self.released.clear();
                if !clip.steps.iter().any(|s| s.wait) {
                    remaining %= clip.duration();
                }
            } else {
                self.finished = true;
                self.playing = false;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Layer, Step};
    fn clip(wait: bool) -> Clip {
        Clip {
            id: "test".into(),
            label: "test".into(),
            category: "test".into(),
            kind: "action".into(),
            verified: true,
            evidence: vec!["fixture".into()],
            notes: String::new(),
            looped: true,
            steps: (0..8)
                .map(|i| Step {
                    ticks: 6.0,
                    wait: wait && i == 3,
                    root_x: i as f64,
                    root_y: 0.0,
                    layers: vec![Layer {
                        sprite: "s".into(),
                        frame: i,
                        x: 0.0,
                        y: 0.0,
                        angle: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        alpha: 1.0,
                        tint: [255; 3],
                        solid: None,
                        color_map: vec![],
                        blend: String::new(),
                        grayscale_fade: 1.0,
                        clip: None,
                        hologram: None,
                    }],
                })
                .collect(),
        }
    }
    #[test]
    fn idle_timing_and_no_drift() {
        let c = clip(false);
        let mut p = Player::new(0, &c);
        p.advance(0.2, 30.0, &c);
        assert_eq!(p.index(&c), 1);
        p.advance(1.4, 30.0, &c);
        assert!(p.tick.abs() < 1e-8);
        for _ in 0..9600 {
            p.advance(1.0 / 60.0, 30.0, &c)
        }
        assert!(p.tick.abs() < 1e-8);
    }
    #[test]
    fn once_returns_position() {
        let c = clip(false);
        let mut p = Player::new(0, &c);
        p.looping = false;
        p.advance(1.4, 30.0, &c);
        assert_eq!(p.root(&c), (7.0, 0.0));
        p.advance(0.2, 30.0, &c);
        assert!(p.finished);
        assert_eq!(p.root(&c), (0.0, 0.0));
        assert_eq!(p.index(&c), 7);
    }
    #[test]
    fn waiting_requires_continue() {
        let c = clip(true);
        let mut p = Player::new(0, &c);
        p.advance(20.0, 30.0, &c);
        assert_eq!(p.waiting, Some(3));
        assert_eq!(p.index(&c), 3);
        p.toggle();
        p.advance(0.2, 30.0, &c);
        assert_eq!(p.index(&c), 5);
    }
    #[test]
    fn pause_seek_and_step() {
        let c = clip(false);
        let mut p = Player::new(0, &c);
        p.toggle();
        p.advance(10.0, 30.0, &c);
        assert_eq!(p.tick, 0.0);
        p.seek(&c, 12.0);
        assert_eq!(p.index(&c), 2);
        p.step(&c, false);
        assert_eq!(p.index(&c), 1);
        p.step(&c, true);
        assert_eq!(p.index(&c), 2);
        assert!(!p.playing);
    }
    #[test]
    fn loop_restores_movement_and_waits() {
        let c = clip(true);
        let mut p = Player::new(0, &c);
        p.advance(0.8, 30.0, &c);
        p.toggle();
        p.advance(0.8, 30.0, &c);
        assert_eq!(p.root(&c), (0.0, 0.0));
        p.advance(0.8, 30.0, &c);
        assert_eq!(p.waiting, Some(3));
    }
}
