use super::*;
use image::{Pixel, Rgba, RgbaImage};
use std::num::NonZeroU32;
use std::sync::OnceLock;

const LETTER_SPACING: u32 = 9;
const LINE_SPACING: u32 = 20;
const LINE_COLUMNS: usize = 37;
const GAME_FPS: u128 = 30;
const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

#[derive(Deserialize)]
struct Glyph {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    offset_x: u32,
}

struct Font {
    atlas: RgbaImage,
    glyphs: HashMap<char, Glyph>,
}

fn font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| Font {
        atlas: image::load_from_memory(include_bytes!("../../assets/ui/aqua-bubble-dotumche.png"))
            .expect("embedded DotumChe atlas")
            .into_rgba8(),
        glyphs: serde_json::from_str(include_str!("../../assets/ui/aqua-bubble-dotumche.json"))
            .expect("embedded DotumChe glyphs"),
    })
}

fn wrap(text: &[char]) -> Vec<&[char]> {
    let mut lines = Vec::new();
    let mut start = 0;
    loop {
        let rest = &text[start..];
        let prefix = &rest[..rest.len().min(LINE_COLUMNS + 1)];
        if let Some(end) = prefix.iter().position(|&ch| ch == '\n') {
            lines.push(&rest[..end]);
            start += end + 1;
        } else if rest.len() <= LINE_COLUMNS {
            lines.push(rest);
            return lines;
        } else if let Some(space) = prefix
            .iter()
            .rposition(|&ch| ch == ' ')
            .filter(|&space| start + space >= 2)
        {
            lines.push(&rest[..space]);
            start += space + 1;
        } else {
            lines.push(&rest[..LINE_COLUMNS]);
            start += LINE_COLUMNS;
        }
    }
}

struct Letter {
    glyph: &'static Glyph,
    x: u32,
    y: u32,
}

struct Layout {
    background: RgbaImage,
    offset: (i32, i32),
    letters: Vec<Letter>,
}

impl Layout {
    fn new(text: &str) -> Result<Self, char> {
        let text: Vec<_> = text.replace("\r\n", "\n").chars().collect();
        let lines = wrap(&text);
        let mut letters = Vec::new();
        for (row, line) in lines.iter().enumerate() {
            for (column, ch) in line.iter().enumerate() {
                let glyph = font().glyphs.get(ch).ok_or(*ch)?;
                letters.push(Letter {
                    glyph,
                    x: 10 + column as u32 * LETTER_SPACING + glyph.offset_x,
                    y: 10 + row as u32 * LINE_SPACING,
                });
            }
        }
        let longest = lines.iter().map(|line| line.len()).max().unwrap() as u32;
        let width = longest * LETTER_SPACING + 10;
        let height = lines.len() as u32 * LINE_SPACING + 5;
        let left = -(width as i32) - 25;
        let top = 3 - height.div_ceil(2) as i32 - 10;
        let offset = (left, top);

        static TAIL: OnceLock<RgbaImage> = OnceLock::new();
        let tail = TAIL.get_or_init(|| {
            image::load_from_memory(include_bytes!("../../assets/ui/aqua-bubble-tail.png"))
                .expect("embedded battle balloon tail")
                .into_rgba8()
        });
        let compact = height < 40;
        let tail_x = (-16 - left) as u32;
        let tail_y = (if compact { -5 } else { -10 } - top) as u32;
        let mut background = RgbaImage::new(tail_x + tail.width(), height + 10);
        for (x, y, pixel) in tail.enumerate_pixels() {
            // GameMaker's half-height sprite samples the odd source rows.
            if !compact || y % 2 == 1 {
                background.put_pixel(tail_x + x, tail_y + if compact { y / 2 } else { y }, *pixel);
            }
        }
        // The original body is two overlapping white rectangles.
        for (x, y, pixel) in background.enumerate_pixels_mut() {
            if (x < width + 10 && (5..height + 5).contains(&y)) || (5..width + 5).contains(&x) {
                *pixel = WHITE;
            }
        }
        Ok(Self {
            background,
            offset,
            letters,
        })
    }

    fn frame(&self, elapsed: Duration) -> render::Frame {
        let ticks = elapsed.as_nanos() * GAME_FPS / 1_000_000_000;
        let visible = (ticks + 1).min(self.letters.len() as u128) as usize;
        let mut frame = self.background.clone();
        let atlas = &font().atlas;
        for letter in &self.letters[..visible] {
            let glyph = letter.glyph;
            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    let alpha = atlas.get_pixel(glyph.x + x, glyph.y + y)[3];
                    if alpha != 0 {
                        frame
                            .get_pixel_mut(letter.x + x, letter.y + y)
                            .blend(&Rgba([0, 0, 0, alpha]));
                    }
                }
            }
        }
        render::Frame {
            width: frame.width() as usize,
            height: frame.height() as usize,
            rgba: frame.into_raw(),
        }
    }
}

pub(super) struct Bubble {
    window: HWND,
    layout: Layout,
    surface: Option<Surface>,
}

impl Bubble {
    /// Prepares one message with the original English battle font.
    pub unsafe fn new(owner: HWND, text: &str) -> windows::core::Result<Self> {
        let layout = Layout::new(text).map_err(|ch| {
            windows::core::Error::new(E_INVALIDARG, format!("原作气泡字体缺少字形 {ch:?}"))
        })?;
        let window = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("STATIC"),
            None,
            WS_POPUP | WS_DISABLED,
            0,
            0,
            0,
            0,
            Some(owner),
            None,
            None,
            None,
        )?;
        Ok(Self {
            window,
            layout,
            surface: None,
        })
    }

    /// Draws the current reveal frame at the tail anchor in physical screen pixels.
    pub unsafe fn draw(
        &mut self,
        anchor: POINT,
        scale: NonZeroU32,
        elapsed: Duration,
    ) -> windows::core::Result<()> {
        let frame = self.layout.frame(elapsed);
        let scale = scale.get() as usize;
        let width = i32::try_from(frame.width * scale).map_err(|_| E_INVALIDARG)?;
        let height = i32::try_from(frame.height * scale).map_err(|_| E_INVALIDARG)?;
        if self
            .surface
            .as_ref()
            .is_none_or(|surface| surface.width != width || surface.height != height)
        {
            self.surface = Some(Surface::new(width, height)?);
        }
        let surface = self.surface.as_ref().unwrap();
        let pixels = std::slice::from_raw_parts_mut(
            surface.bits.cast::<[u8; 4]>(),
            width as usize * height as usize,
        );
        frame.write_bgra(pixels, scale);
        let dst = POINT {
            x: anchor.x + self.layout.offset.0 * scale as i32,
            y: anchor.y + self.layout.offset.1 * scale as i32,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            self.window,
            None,
            Some(&dst),
            Some(&size),
            Some(surface.dc),
            Some(&POINT::default()),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )?;
        let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
        Ok(())
    }
}

impl Drop for Bubble {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(frame: &render::Frame, x: usize, y: usize) -> &[u8] {
        let offset = (y * frame.width + x) * 4;
        &frame.rgba[offset..offset + 4]
    }

    #[test]
    fn body_and_tail_follow_the_battle_dimensions() {
        let short = Layout::new("AB").unwrap();
        let frame = short.frame(Duration::ZERO);
        assert_eq!(
            (frame.width, frame.height, short.offset),
            (57, 35, (-53, -20))
        );
        assert_eq!(pixel(&frame, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&frame, 5, 0), WHITE.0);
        assert_eq!(pixel(&frame, 0, 5), WHITE.0);
        assert_eq!(pixel(&frame, 56, 19), WHITE.0);
        assert_eq!(pixel(&frame, 56, 18), [0, 0, 0, 0]);

        let two_lines = Layout::new("AB\nC").unwrap();
        let frame = two_lines.frame(Duration::ZERO);
        assert_eq!(
            (frame.width, frame.height, two_lines.offset),
            (57, 55, (-53, -30))
        );
        assert_eq!(pixel(&frame, 56, 29), WHITE.0);
        assert_eq!(pixel(&frame, 37, 20), WHITE.0);
    }

    #[test]
    fn original_glyphs_reveal_one_per_game_tick_without_resizing() {
        let layout = Layout::new("AA\nA").unwrap();
        let first = layout.frame(Duration::ZERO);
        let second = layout.frame(Duration::from_millis(34));
        let third = layout.frame(Duration::from_millis(67));
        assert_eq!((first.width, first.height), (third.width, third.height));
        let black = [0, 0, 0, 255];
        assert_eq!(pixel(&first, 14, 12), black);
        assert_eq!(pixel(&first, 23, 12), WHITE.0);
        assert_eq!(pixel(&second, 23, 12), black);
        assert_eq!(pixel(&second, 14, 32), WHITE.0);
        assert_eq!(pixel(&third, 14, 32), black);
    }

    #[test]
    fn wraps_at_the_original_column_limit_and_checks_glyph_coverage() {
        let text = format!("{} word", "A".repeat(35));
        let layout = Layout::new(&text).unwrap();
        assert_eq!((layout.letters[35].x, layout.letters[35].y), (11, 30));
        let layout = Layout::new(&"A".repeat(38)).unwrap();
        assert_eq!((layout.letters[37].x, layout.letters[37].y), (11, 30));
        assert!(matches!(Layout::new("你"), Err('你')));
    }
}
