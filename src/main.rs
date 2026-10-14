#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod catalog;
mod placement;
#[cfg(windows)]
mod platform;
mod player;
mod render;
mod wander;

fn main() {
    let result = run();
    if let Err(error) = result {
        eprintln!("AQUA: {error}");
        #[cfg(windows)]
        platform::show_error(&error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    if !args
        .iter()
        .any(|a| a == "--validate" || a == "--smoke-test")
        && platform::show_existing(args.iter().any(|a| a == "--debug"))
    {
        return Ok(());
    }
    let catalog = catalog::Catalog::load()?;
    if args.iter().any(|a| a == "--validate") {
        let idle = &catalog.clips[catalog.idle_index()];
        if (idle.duration() - 48.0).abs() > 1e-8 {
            return Err("默认晃动必须为48个原作tick（1.6秒）".into());
        }
        println!(
            "{} resources / {} PNG frames / {} original action presets; default idle: 1.6s; source {}",
            catalog.sprites.len(),
            catalog.images.len(),
            catalog.clips.iter().filter(|c| c.kind == "action").count(),
            catalog.source_hash
        );
        return Ok(());
    }
    #[cfg(windows)]
    {
        platform::run(
            catalog,
            args.iter().any(|a| a == "--debug"),
            args.iter().any(|a| a == "--smoke-test"),
        )
    }
    #[cfg(not(windows))]
    {
        let _ = catalog;
        Err("桌宠窗口目前支持Windows 10/11。素材与动画测试可跨平台运行。".into())
    }
}
