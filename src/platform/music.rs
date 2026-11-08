use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use windows::{
    Media::{
        Control::{
            GlobalSystemMediaTransportControlsSessionManager as SessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
        },
        MediaPlaybackType,
    },
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

const CHECK_INTERVAL: Duration = Duration::from_millis(200);
const MAX_CHECK_GAP: Duration = Duration::from_secs(1);
const MUSIC_THRESHOLD: Duration = Duration::from_secs(10);

struct Playback {
    last_check: Instant,
    started: Option<Instant>,
}

impl Playback {
    fn observe(&mut self, now: Instant, playing: bool) {
        if !playing || now.duration_since(self.last_check) > MAX_CHECK_GAP {
            self.started = None;
        }
        if playing {
            self.started.get_or_insert(now);
        }
        self.last_check = now;
    }

    fn confirmed(&self, now: Instant) -> bool {
        now.duration_since(self.last_check) <= MAX_CHECK_GAP
            && self
                .started
                .is_some_and(|start| self.last_check.duration_since(start) > MUSIC_THRESHOLD)
    }
}

pub(super) struct Monitor {
    playback: Arc<Mutex<Playback>>,
    _stop: mpsc::Sender<()>,
}

impl Monitor {
    pub fn new() -> Self {
        let playback = Arc::new(Mutex::new(Playback {
            last_check: Instant::now(),
            started: None,
        }));
        let shared = Arc::clone(&playback);
        let (sender, stop) = mpsc::channel();
        // Windows can block while reading other apps. Keep those calls off the window thread.
        std::thread::spawn(move || {
            if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
                return;
            }
            let mut manager = None;
            loop {
                if manager.is_none() {
                    manager = SessionManager::RequestAsync()
                        .and_then(|request| request.join())
                        .ok();
                }
                let playing = manager.as_ref().and_then(|m| has_playing_music(m).ok());
                shared
                    .lock()
                    .unwrap()
                    .observe(Instant::now(), playing.unwrap_or(false));
                let delay = if playing.is_some() {
                    CHECK_INTERVAL
                } else {
                    manager = None;
                    Duration::from_secs(2)
                };
                if stop.recv_timeout(delay) != Err(mpsc::RecvTimeoutError::Timeout) {
                    break;
                }
            }
            drop(manager);
            unsafe { RoUninitialize() };
        });
        Self {
            playback,
            _stop: sender,
        }
    }

    pub fn playing(&self, now: Instant) -> bool {
        self.playback.lock().unwrap().confirmed(now)
    }
}

fn has_playing_music(manager: &SessionManager) -> windows::core::Result<bool> {
    let sessions = manager.GetSessions()?;
    Ok((0..sessions.Size()?).any(|index| {
        sessions
            .GetAt(index)
            .and_then(|session| session.GetPlaybackInfo())
            .is_ok_and(|info| {
                matches!(info.PlaybackStatus(), Ok(PlaybackStatus::Playing))
                    && matches!(
                        info.PlaybackType().and_then(|kind| kind.Value()),
                        Ok(MediaPlaybackType::Music)
                    )
            })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_requires_more_than_ten_seconds_and_resets_on_pause_or_gap() {
        let start = Instant::now();
        let mut playback = Playback {
            last_check: start,
            started: None,
        };
        for ms in (0..=10_000).step_by(200) {
            let now = start + Duration::from_millis(ms);
            playback.observe(now, true);
            assert!(!playback.confirmed(now));
        }
        let confirmed = start + Duration::from_millis(10_200);
        playback.observe(confirmed, true);
        assert!(playback.confirmed(confirmed));
        assert!(!playback.confirmed(confirmed + MAX_CHECK_GAP + CHECK_INTERVAL));
        playback.observe(confirmed + CHECK_INTERVAL, false);
        assert!(!playback.confirmed(confirmed + CHECK_INTERVAL));

        let resumed = confirmed + 2 * CHECK_INTERVAL;
        for ms in (0..=9_800).step_by(200) {
            playback.observe(resumed + Duration::from_millis(ms), true);
        }
        let after_gap = resumed + Duration::from_secs(12);
        playback.observe(after_gap, true);
        assert!(!playback.confirmed(after_gap));
        assert_eq!(playback.started, Some(after_gap));
    }
}
