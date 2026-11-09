use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
    core::PCWSTR,
};

pub(super) fn menu_move() {
    play(include_bytes!("../../assets/sounds/snd_menumove.wav"));
}

pub(super) fn menu_confirm() {
    play(include_bytes!("../../assets/sounds/snd_select.wav"));
}

fn play(wave: &'static [u8]) {
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(wave.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}
