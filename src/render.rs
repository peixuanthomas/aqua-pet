use crate::catalog::{Catalog, Clip, Hologram, Layer, Step};

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
impl Bounds {
    pub fn empty() -> Self {
        Self {
            left: f64::INFINITY,
            top: f64::INFINITY,
            right: f64::NEG_INFINITY,
            bottom: f64::NEG_INFINITY,
        }
    }
    pub fn point(&mut self, x: f64, y: f64) {
        self.left = self.left.min(x);
        self.top = self.top.min(y);
        self.right = self.right.max(x);
        self.bottom = self.bottom.max(y);
    }
    pub fn width(self) -> f64 {
        (self.right - self.left).max(1.0)
    }
    pub fn height(self) -> f64 {
        (self.bottom - self.top).max(1.0)
    }
    fn round(self) -> Self {
        Self {
            left: self.left.floor() - 1.0,
            top: self.top.floor() - 1.0,
            right: self.right.ceil() + 1.0,
            bottom: self.bottom.ceil() + 1.0,
        }
    }
}

fn layer_bounds(catalog: &Catalog, layer: &Layer) -> Bounds {
    let s = &catalog.sprites[&layer.sprite];
    let mut bounds = Bounds::empty();
    let (sin, cos) = layer.angle.to_radians().sin_cos();
    for (x, y) in [
        (0.0, 0.0),
        (s.width as f64, 0.0),
        (0.0, s.height as f64),
        (s.width as f64, s.height as f64),
    ] {
        let dx = (x - s.origin_x) * layer.scale_x;
        let dy = (y - s.origin_y) * layer.scale_y;
        bounds.point(layer.x + cos * dx + sin * dy, layer.y - sin * dx + cos * dy);
    }
    bounds
}

pub fn clip_bounds(catalog: &Catalog, clip: &Clip, include_motion: bool) -> Bounds {
    let mut b = Bounds::empty();
    for step in &clip.steps {
        for layer in &step.layers {
            let lb = layer_bounds(catalog, layer);
            let (x, y) = if include_motion {
                (step.root_x, step.root_y)
            } else {
                (0.0, 0.0)
            };
            b.point(lb.left + x, lb.top + y);
            b.point(lb.right + x, lb.bottom + y);
            // Include the final frame at the returned origin as well.
            b.point(lb.left, lb.top);
            b.point(lb.right, lb.bottom);
        }
    }
    b.round()
}

#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
impl Frame {
    pub fn bgra_premultiplied(&self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.rgba.len());
        for p in self.rgba.as_chunks::<4>().0 {
            let a = p[3] as u32;
            pixels.extend_from_slice(&[
                ((p[2] as u32 * a + 127) / 255) as u8,
                ((p[1] as u32 * a + 127) / 255) as u8,
                ((p[0] as u32 * a + 127) / 255) as u8,
                p[3],
            ]);
        }
        pixels
    }
    pub fn on_background(&self, kind: usize) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.rgba.len());
        for (i, p) in self.rgba.as_chunks::<4>().0.iter().enumerate() {
            let bg = match kind {
                0 => [27u32, 34, 44],
                1 => [240, 240, 234],
                _ => {
                    if ((i % self.width) / 12 + (i / self.width) / 12).is_multiple_of(2) {
                        [210, 216, 222]
                    } else {
                        [166, 178, 188]
                    }
                }
            };
            let a = p[3] as u32;
            let mut c = [0u8; 4];
            for j in 0..3 {
                c[2 - j] = ((p[j] as u32 * a + bg[j] * (255 - a) + 127) / 255) as u8;
            }
            c[3] = 255;
            pixels.extend_from_slice(&c);
        }
        pixels
    }
}

pub fn render(
    catalog: &Catalog,
    step: &Step,
    bounds: Bounds,
    scale: f64,
    include_motion: bool,
) -> Frame {
    render_internal(catalog, step, bounds, scale, include_motion, None)
}

pub fn render_preview(
    catalog: &Catalog,
    step: &Step,
    bounds: Bounds,
    scale: f64,
    background: usize,
) -> Frame {
    render_internal(catalog, step, bounds, scale, true, Some(background))
}

// Translation of shd_chromatic_reflection. GLSL fract is always nonnegative.
fn hologram(h: &Hologram, x: f64, y: f64) -> ([f64; 3], f64) {
    let x = h.origin_x + 2.0 * x;
    let y = h.origin_y - 2.0 * y;
    let fract = |v: f64| v - v.floor();
    let ux = fract(y * 0.006 + 0.5) - 0.5;
    let uy = fract(-x * 0.006 + 0.5) - 0.5;
    #[allow(clippy::approx_constant)] // Exact literal from the original shader.
    let v = (fract(uy.atan2(ux) / 3.141592) * 2.0 - 1.0).abs();
    let hue = 0.5 + fract((h.time_x + (x + y) * 0.01) * 0.5);
    let rgb = [0.0, 2.0 / 3.0, 1.0 / 3.0]
        .map(|offset| ((fract(hue + offset) * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0) * 3.7);
    (rgb, v * 0.5)
}

fn render_internal(
    catalog: &Catalog,
    step: &Step,
    bounds: Bounds,
    scale: f64,
    include_motion: bool,
    background: Option<usize>,
) -> Frame {
    let width = (bounds.width() * scale).ceil().max(1.0) as usize;
    let height = (bounds.height() * scale).ceil().max(1.0) as usize;
    let mut output = Frame {
        width,
        height,
        rgba: vec![0; width * height * 4],
    };
    if let Some(kind) = background {
        output.rgba = output.on_background(kind);
        for p in output.rgba.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
    }
    let (root_x, root_y) = if include_motion {
        (step.root_x, step.root_y)
    } else {
        (0.0, 0.0)
    };
    for layer in &step.layers {
        if layer.alpha <= 0.0 || layer.scale_x == 0.0 || layer.scale_y == 0.0 {
            continue;
        }
        let sprite = &catalog.sprites[&layer.sprite];
        let image = &catalog.images[&sprite.frames[layer.frame]];
        let lb = layer_bounds(catalog, layer);
        let x0 = ((lb.left + root_x - bounds.left) * scale).floor().max(0.0) as usize;
        let y0 = ((lb.top + root_y - bounds.top) * scale).floor().max(0.0) as usize;
        let x1 = ((lb.right + root_x - bounds.left) * scale)
            .ceil()
            .clamp(0.0, width as f64) as usize;
        let y1 = ((lb.bottom + root_y - bounds.top) * scale)
            .ceil()
            .clamp(0.0, height as f64) as usize;
        let (sin, cos) = layer.angle.to_radians().sin_cos();
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = (x as f64 + 0.5) / scale + bounds.left - layer.x - root_x;
                let dy = (y as f64 + 0.5) / scale + bounds.top - layer.y - root_y;
                let scene_x = dx + layer.x;
                let scene_y = dy + layer.y;
                if layer.clip.is_some_and(|r| {
                    scene_x < r[0] || scene_y < r[1] || scene_x >= r[2] || scene_y >= r[3]
                }) {
                    continue;
                }
                let sx = ((cos * dx - sin * dy) / layer.scale_x + sprite.origin_x).floor();
                let sy = ((sin * dx + cos * dy) / layer.scale_y + sprite.origin_y).floor();
                if sx < 0.0 || sy < 0.0 || sx >= image.width() as f64 || sy >= image.height() as f64
                {
                    continue;
                }
                let mut p = image.get_pixel(sx as u32, sy as u32).0;
                let mut a = (p[3] as f64 * layer.alpha).round() as u32;
                if a == 0 {
                    continue;
                }
                if let Some(solid) = layer.solid {
                    p[..3].copy_from_slice(&solid);
                } else if let Some(map) = layer.color_map.iter().find(|m| p[..3] == m[..3]) {
                    p[..3].copy_from_slice(&map[3..]);
                }
                let gray = p[0] as f64 * 0.21 + p[1] as f64 * 0.72 + p[2] as f64 * 0.07;
                for channel in &mut p[..3] {
                    *channel = (*channel as f64 * layer.grayscale_fade
                        + gray * (1.0 - layer.grayscale_fade))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
                for (channel, tint) in p[..3].iter_mut().zip(layer.tint) {
                    *channel = ((*channel as u32 * tint as u32 + 127) / 255) as u8;
                }
                if let Some(h) = &layer.hologram {
                    let (rgb, intensity) = hologram(h, scene_x, scene_y);
                    let source_alpha = p[3] as f64 / 255.0;
                    for j in 0..3 {
                        p[j] = (rgb[j] * intensity * source_alpha * 255.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                    a = (layer.alpha * intensity * source_alpha * 255.0).round() as u32;
                    if a == 0 {
                        continue;
                    }
                }
                let out = &mut output.rgba[(y * width + x) * 4..][..4];
                let old_alpha = out[3] as u32;
                let alpha = if layer.blend == "add" {
                    (a + old_alpha).min(255)
                } else {
                    a + (old_alpha * (255 - a) + 127) / 255
                };
                for j in 0..3 {
                    let prior = if layer.blend == "add" {
                        out[j] as u32 * old_alpha
                    } else {
                        (out[j] as u32 * old_alpha * (255 - a) + 127) / 255
                    };
                    out[j] = ((p[j] as u32 * a + prior + alpha / 2) / alpha).min(255) as u8;
                }
                out[3] = alpha as u8;
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    #[test]
    fn pet_and_preview_share_frame_and_motion_after_controls() {
        let c = Catalog::load().unwrap();
        let mut clip = c.clips[c.idle_index()].clone();
        for (i, s) in clip.steps.iter_mut().enumerate() {
            s.root_x = i as f64 * 3.0;
            s.root_y = -(i as f64);
        }
        let b = clip_bounds(&c, &clip, true);
        let mut p = Player::new(0, &clip);
        for operation in 0..8 {
            match operation {
                0 => p.advance(0.7, 30.0, &clip),
                1 => p.toggle(),
                2 => p.advance(2.0, 30.0, &clip),
                3 => p.step(&clip, true),
                4 => p.seek(&clip, 12.0),
                5 => p.restart(),
                6 => {
                    p.looping = false;
                    p.advance(2.0, 30.0, &clip);
                }
                _ => {
                    p.select(0, &clip);
                    p.advance(3.3, 30.0, &clip);
                }
            }
            let mut step = clip.steps[p.index(&clip)].clone();
            let root = p.root(&clip);
            step.root_x = root.0;
            step.root_y = root.1;
            let pet = render(&c, &step, b, 1.0, false);
            let preview = render(&c, &step, b, 1.0, true);
            for y in 0..pet.height {
                for x in 0..pet.width {
                    let source = &pet.rgba[(y * pet.width + x) * 4..][..4];
                    if source[3] > 0 {
                        let px = (x as i32 + root.0 as i32) as usize;
                        let py = (y as i32 + root.1 as i32) as usize;
                        assert_eq!(
                            source,
                            &preview.rgba[(py * preview.width + px) * 4..][..4],
                            "operation {operation}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn clipping_and_additive_preview_do_not_paint_outside_their_area() {
        let c = Catalog::load().unwrap();
        let clip = &c.clips[c.idle_index()];
        let b = clip_bounds(&c, clip, false);
        let mut step = clip.steps[0].clone();
        step.layers[0].clip = Some([0.0, 0.0, 10.0, 40.0]);
        step.layers[0].solid = Some([240, 0, 0]);
        step.layers[0].alpha = 0.5;
        step.layers[0].blend = "add".into();
        let f = render_preview(&c, &step, b, 1.0, 0);
        let mut red_pixels = 0;
        for (i, p) in f.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(p[1], 34);
            assert_eq!(p[2], 44);
            assert_eq!(p[3], 255);
            assert!(p[0] >= 27);
            if p[0] > 27 {
                red_pixels += 1;
                let x = (i % f.width) as f64 + b.left;
                assert!((0.0..10.0).contains(&x));
            }
        }
        assert!(red_pixels > 0);
    }
    #[test]
    fn transparent_bgra_and_background() {
        let f = Frame {
            width: 2,
            height: 1,
            rgba: vec![100, 200, 50, 128, 99, 44, 33, 0],
        };
        assert_eq!(f.bgra_premultiplied(), vec![25, 100, 50, 128, 0, 0, 0, 0]);
        assert_eq!(&f.on_background(0)[4..], &[44, 34, 27, 255]);
    }
    #[test]
    fn all_assets_and_frames_render() {
        let c = Catalog::load().unwrap();
        assert_eq!(c.images.len(), 346);
        for clip in &c.clips {
            let b = clip_bounds(&c, clip, false);
            assert!(b.width().is_finite() && b.height().is_finite());
            for step in &clip.steps {
                let f = render(&c, step, b, 1.0, false);
                assert_eq!(f.rgba.len(), f.width * f.height * 4);
            }
        }
    }
    #[test]
    fn idle_preserves_pixels_at_integer_scale() {
        let c = Catalog::load().unwrap();
        let clip = &c.clips[c.idle_index()];
        let b = clip_bounds(&c, clip, false);
        let step = &clip.steps[0];
        let a = render(&c, step, b, 1.0, false);
        let big = render(&c, step, b, 4.0, false);
        for y in 0..a.height {
            for x in 0..a.width {
                assert_eq!(
                    &a.rgba[(y * a.width + x) * 4..][..4],
                    &big.rgba[((y * 4) * big.width + x * 4) * 4..][..4]
                );
            }
        }
    }
}
