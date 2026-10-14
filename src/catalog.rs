use image::RgbaImage;
use serde::Deserialize;
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/images.rs"));

#[derive(Deserialize)]
pub struct SpriteFile {
    pub source_sha256: String,
    pub game_fps: f64,
    pub sprites: Vec<Sprite>,
}

#[derive(Deserialize)]
pub struct Sprite {
    pub id: String,
    pub category: String,
    pub width: u32,
    pub height: u32,
    pub origin_x: f64,
    pub origin_y: f64,
    pub playback_speed: f64,
    pub playback_speed_type: String,
    pub frames: Vec<String>,
}

fn one() -> f64 {
    1.0
}
fn white() -> [u8; 3] {
    [255, 255, 255]
}

#[derive(Clone, Debug, Deserialize)]
pub struct Hologram {
    pub time_x: f64,
    pub origin_x: f64,
    pub origin_y: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Layer {
    pub sprite: String,
    pub frame: usize,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub angle: f64,
    #[serde(default = "one")]
    pub scale_x: f64,
    #[serde(default = "one")]
    pub scale_y: f64,
    #[serde(default = "one")]
    pub alpha: f64,
    #[serde(default = "white")]
    pub tint: [u8; 3],
    #[serde(default)]
    pub solid: Option<[u8; 3]>,
    #[serde(default)]
    pub color_map: Vec<[u8; 6]>,
    #[serde(default)]
    pub blend: String,
    #[serde(default = "one")]
    pub grayscale_fade: f64,
    #[serde(default)]
    pub clip: Option<[f64; 4]>,
    #[serde(default)]
    pub hologram: Option<Hologram>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Step {
    pub ticks: f64,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub wait: bool,
    #[serde(default)]
    pub root_x: f64,
    #[serde(default)]
    pub root_y: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Clip {
    pub id: String,
    pub label: String,
    pub category: String,
    pub kind: String,
    pub verified: bool,
    pub evidence: Vec<String>,
    pub notes: String,
    pub looped: bool,
    pub steps: Vec<Step>,
}

impl Clip {
    pub fn duration(&self) -> f64 {
        self.steps.iter().map(|s| s.ticks).sum()
    }
    pub fn step_at(&self, tick: f64) -> usize {
        let mut end = 0.0;
        for (i, step) in self.steps.iter().enumerate() {
            end += step.ticks;
            if tick < end - 1e-8 {
                return i;
            }
        }
        self.steps.len() - 1
    }
    pub fn step_start(&self, index: usize) -> f64 {
        self.steps.iter().take(index).map(|s| s.ticks).sum()
    }
}

pub struct Catalog {
    pub sprites: HashMap<String, Sprite>,
    pub images: HashMap<String, RgbaImage>,
    pub clips: Vec<Clip>,
    pub source_hash: String,
    pub game_fps: f64,
}

impl Catalog {
    pub fn load() -> Result<Self, String> {
        let file: SpriteFile = serde_json::from_str(include_str!("../assets/sprites.json"))
            .map_err(|e| e.to_string())?;
        let mut images = HashMap::new();
        for &(name, bytes) in IMAGES {
            images.insert(
                name.to_string(),
                image::load_from_memory(bytes)
                    .map_err(|e| format!("{name}: {e}"))?
                    .into_rgba8(),
            );
        }
        let mut clips: Vec<Clip> =
            serde_json::from_str(include_str!("../assets/character-actions.json"))
                .map_err(|e| format!("角色动作: {e}"))?;
        let composites: Vec<Clip> =
            serde_json::from_str(include_str!("../assets/composite-actions.json"))
                .map_err(|e| format!("组合动作: {e}"))?;
        clips.extend(composites);
        for sprite in &file.sprites {
            let unused = sprite.category.contains("Unused");
            let fps = if sprite.playback_speed_type == "FramesPerSecond" {
                sprite.playback_speed
            } else {
                sprite.playback_speed * file.game_fps
            };
            let ticks = if fps > 0.0 { file.game_fps / fps } else { 1.0 };
            clips.push(Clip {
                id: format!("resource/{}", sprite.id),
                label: sprite.id.clone(), category: format!("素材 / {}", sprite.category), kind: "resource".into(), verified: false,
                evidence: vec![format!("data.win / {}；原图 {}×{}；原点 {},{}", sprite.id, sprite.width, sprite.height, sprite.origin_x, sprite.origin_y)],
                notes: format!("{}资源浏览：按游戏资源默认速度 {:.3} 帧/秒展示，不代表剧情中的实际播放方式。实际动作请选择「原作动作」。", if unused { "未使用素材。" } else { "" }, fps),
                looped: sprite.frames.len() > 1,
                steps: (0..sprite.frames.len()).map(|frame| Step { ticks, wait: false, root_x: 0.0, root_y: 0.0, layers: vec![Layer { sprite: sprite.id.clone(), frame, x: 0.0, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0,tint:white(),solid:None,color_map:vec![],blend:String::new(),grayscale_fade:1.0,clip:None,hologram:None }] }).collect(),
            });
        }
        let catalog = Self {
            sprites: file
                .sprites
                .into_iter()
                .map(|s| (s.id.clone(), s))
                .collect(),
            images,
            clips,
            source_hash: file.source_sha256,
            game_fps: file.game_fps,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.sprites.len() != 94 || self.images.len() != 346 {
            return Err("素材数量不完整，应为94组346张。".into());
        }
        for s in self.sprites.values() {
            for name in &s.frames {
                let image = self
                    .images
                    .get(name)
                    .ok_or_else(|| format!("缺少图片 {name}"))?;
                if image.width() != s.width || image.height() != s.height {
                    return Err(format!("图片尺寸不符 {name}"));
                }
            }
        }
        let mut ids = std::collections::HashSet::new();
        for c in &self.clips {
            if !ids.insert(&c.id) {
                return Err(format!("重复动作 {}", c.id));
            }
            if c.steps.is_empty()
                || c.steps
                    .iter()
                    .any(|s| !s.ticks.is_finite() || s.ticks <= 0.0)
                || c.steps.iter().all(|s| s.layers.is_empty())
            {
                return Err(format!("动作数据无效 {}", c.id));
            }
            if c.verified && c.evidence.is_empty() {
                return Err(format!("动作缺少依据 {}", c.id));
            }
            for step in &c.steps {
                if !step.root_x.is_finite() || !step.root_y.is_finite() {
                    return Err(format!("位移无效 {}", c.id));
                }
                for l in &step.layers {
                    let s = self
                        .sprites
                        .get(&l.sprite)
                        .ok_or_else(|| format!("{}引用了缺少的资源 {}", c.id, l.sprite))?;
                    if l.frame >= s.frames.len()
                        || ![l.x, l.y, l.angle, l.scale_x, l.scale_y, l.alpha]
                            .iter()
                            .all(|v| v.is_finite())
                        || !(0.0..=1.0).contains(&l.alpha)
                        || !(0.0..=1.0).contains(&l.grayscale_fade)
                        || l.clip.is_some_and(|r| {
                            !r.iter().all(|v| v.is_finite()) || r[0] > r[2] || r[1] > r[3]
                        })
                        || l.hologram.as_ref().is_some_and(|h| {
                            ![h.time_x, h.origin_x, h.origin_y]
                                .iter()
                                .all(|v| v.is_finite())
                        })
                    {
                        return Err(format!("动作帧无效 {} / {}", c.id, l.sprite));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn idle_index(&self) -> usize {
        self.clips
            .iter()
            .position(|c| c.id == "idle")
            .unwrap_or_else(|| {
                self.clips
                    .iter()
                    .position(|c| {
                        c.kind == "action"
                            && c.steps.len() == 8
                            && c.steps[0].layers[0].sprite == "spr_enemy_aqua_idle"
                            && (c.duration() - 48.0).abs() < 0.01
                    })
                    .expect("Missing verified default idle")
            })
    }
}
