/// `silo-client register <path> [--name "Display Name"]`
///
/// Registers an already-extracted game directory with Steam — same VDF writes
/// the pull flow does, but skips rsync and extraction.
use std::path::{Path, PathBuf};

use anyhow::Context;
use tracing::info;

use crate::{
    extract::SiloManifest,
    steam::{self, appid, shortcuts::Shortcut},
};

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let (game_dir, display_name, no_launch) = parse_args(args)?;

    let game_dir = game_dir
        .canonicalize()
        .with_context(|| format!("path not found: {}", game_dir.display()))?;

    let display_name = display_name.unwrap_or_else(|| {
        game_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });

    let manifest = read_manifest(&game_dir)?;

    let exe_abs = game_dir.join(&manifest.exe_path);
    anyhow::ensure!(
        exe_abs.exists(),
        "exe not found: {} (from silo.toml exe_path)",
        exe_abs.display()
    );

    let exe_str = exe_abs.to_string_lossy().to_string();
    let start_dir = exe_abs
        .parent()
        .unwrap_or(&game_dir)
        .to_string_lossy()
        .to_string();

    let app_id = appid::compute(&exe_str, &display_name);
    let shortcut = Shortcut {
        app_id,
        app_name: display_name.clone(),
        exe: exe_str.clone(),
        start_dir,
        icon: String::new(),
        launch_options: manifest.launch_args.clone(),
    };

    let steam_root = steam::find_steam_root_at(
        std::env::var("SILO_STEAM_ROOT").ok().as_deref().map(Path::new),
    )?;

    if steam::is_steam_running() {
        info!("Closing Steam to update shortcuts…");
        // Run shutdown synchronously since we're not in an async context.
        std::process::Command::new("steam")
            .arg("-shutdown")
            .status()
            .context("failed to run steam -shutdown")?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            if !steam::is_steam_running() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!("Steam did not exit within 60 seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        info!("Steam closed");
    }

    steam::add_shortcut(&steam_root, &shortcut, &manifest.proton_version)?;

    let run_game_id = appid::to_run_game_id(app_id);
    info!("Registered \"{display_name}\"");
    info!("  exe:    {exe_str}");
    info!("  proton: {}", manifest.proton_version);
    info!("  launch: steam://rungameid/{run_game_id}");

    if !no_launch {
        steam::launch_steam();
        info!("Steam relaunched");
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> anyhow::Result<(PathBuf, Option<String>, bool)> {
    let mut path: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut no_launch = false;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--name" | "-n" => {
                name = Some(iter.next().context("--name requires a value")?);
            }
            "--no-launch" => {
                no_launch = true;
            }
            _ if path.is_none() => {
                path = Some(PathBuf::from(arg));
            }
            other => anyhow::bail!("unexpected argument: {other}"),
        }
    }

    let path = path.context("usage: silo-client register <path> [--name \"Display Name\"] [--no-launch]")?;
    Ok((path, name, no_launch))
}

fn read_manifest(game_dir: &Path) -> anyhow::Result<SiloManifest> {
    let toml_path = game_dir.join("silo.toml");
    anyhow::ensure!(
        toml_path.exists(),
        "silo.toml not found in {}",
        game_dir.display()
    );
    let content = std::fs::read_to_string(&toml_path)?;
    toml::from_str(&content).context("failed to parse silo.toml")
}
