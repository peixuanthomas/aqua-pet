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
}
