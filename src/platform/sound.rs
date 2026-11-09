use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
    core::PCWSTR,
};

const MENU_MOVE: &[u8] = include_bytes!("../../assets/sounds/snd_menumove.wav");

pub fn menu_move() {
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(MENU_MOVE.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "plays the embedded menu sound on the default audio device"]
    fn embedded_menu_sound_is_valid() {
        let played = unsafe {
            PlaySoundW(
                PCWSTR(MENU_MOVE.as_ptr().cast()),
                None,
                SND_MEMORY | SND_NODEFAULT,
            )
        };
        assert!(played.as_bool());
    }
}
