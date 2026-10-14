use image::{
    RgbaImage,
    codecs::ico::{IcoEncoder, IcoFrame},
    imageops::{self, FilterType},
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut paths: Vec<_> = fs::read_dir(root.join("assets/sprites"))
        .expect("Run tools/extract_assets.ps1 first")
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    paths.sort();
    let mut generated = String::from("pub static IMAGES: &[(&str, &[u8])] = &[\n");
    for path in paths {
        let name = format!("sprites/{}", path.file_name().unwrap().to_str().unwrap());
        generated.push_str(&format!(
            "({name:?}, include_bytes!({:?})),\n",
            path.to_str().unwrap()
        ));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("images.rs"),
        generated,
    )
    .unwrap();

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        let source = image::open(root.join("assets/sprites/spr_aqua_sleevewave_0.png"))
            .expect("Failed to read the application icon")
            .into_rgba8();
        let side = source.width().max(source.height());
        let mut square = RgbaImage::new(side, side);
        imageops::overlay(
            &mut square,
            &source,
            ((side - source.width()) / 2) as i64,
            ((side - source.height()) / 2) as i64,
        );
        let frames: Vec<_> = [16, 24, 32, 48, 64, 256]
            .into_iter()
            .map(|size| {
                let pixels = imageops::resize(&square, size, size, FilterType::Nearest);
                IcoFrame::as_png(&pixels, size, size, image::ExtendedColorType::Rgba8).unwrap()
            })
            .collect();
        let icon = out.join("aqua.ico");
        IcoEncoder::new(fs::File::create(&icon).unwrap())
            .encode_images(&frames)
            .expect("Failed to encode the application icon");
        let resource = out.join("aqua.rc");
        fs::write(
            &resource,
            format!(
                "AQUA_ICON ICON \"{}\"\n",
                icon.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        embed_resource::compile(&resource, embed_resource::NONE)
            .manifest_required()
            .expect("Failed to embed the application icon");
    }
}
