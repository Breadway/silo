use std::{
    ffi::OsStr,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
use tracing::info;
use walkdir::WalkDir;

use crate::config::Config;

pub fn run(args: Vec<String>, cfg: &Config) -> anyhow::Result<()> {
    let archive_path = parse_args(args)?;
    let archive_path = archive_path
        .canonicalize()
        .with_context(|| format!("not found: {}", archive_path.display()))?;

    let game_name = derive_game_name(&archive_path);
    println!("Game name: {game_name}");

    let tmp_dir = std::env::temp_dir().join(format!("silo-norm-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir)?;

    let result = normalize_inner(&archive_path, &tmp_dir, &game_name, cfg);
    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

fn normalize_inner(
    archive_path: &Path,
    tmp_dir: &Path,
    game_name: &str,
    cfg: &Config,
) -> anyhow::Result<()> {
    // ── 1. Extract ────────────────────────────────────────────────────────────
    let extract_dir = tmp_dir.join("extracted");
    std::fs::create_dir_all(&extract_dir)?;
    println!("Extracting {}...", archive_path.file_name().unwrap_or_default().to_string_lossy());
    extract_archive(archive_path, &extract_dir)?;

    // ── 2. Find root dir ──────────────────────────────────────────────────────
    let root_dir = find_root(&extract_dir);
    info!("root: {}", root_dir.display());

    // ── 3. Find and pick exe ──────────────────────────────────────────────────
    let mut candidates = find_game_exes(&root_dir, game_name);
    anyhow::ensure!(!candidates.is_empty(), "no .exe files found — cannot determine exe_path");

    let exe_rel = if candidates.len() == 1 {
        let rel = candidates.remove(0);
        println!("Exe (auto): {}", rel.display());
        rel
    } else {
        println!("\nExecutable candidates:");
        for (i, p) in candidates.iter().enumerate() {
            println!("  {}) {}", i + 1, p.display());
        }
        print!("Select [1]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let idx = line.trim().parse::<usize>().unwrap_or(1).saturating_sub(1);
        let idx = idx.min(candidates.len() - 1);
        candidates.remove(idx)
    };

    // ── 4. Proton version ─────────────────────────────────────────────────────
    println!("\nProton version:");
    println!("  1) GE-Proton10-34  (recommended)");
    println!("  2) proton_9        (Steam stable)");
    print!("Choice [1]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let proton = match line.trim() {
        "2" => "proton_9",
        _ => "GE-Proton10-34",
    };

    // ── 5. Write silo.toml ────────────────────────────────────────────────────
    let exe_str = exe_rel.to_string_lossy().replace('\\', "/");
    std::fs::write(
        root_dir.join("silo.toml"),
        format!("exe_path = \"{exe_str}\"\nlaunch_args = \"\"\nproton_version = \"{proton}\"\n"),
    )?;
    println!("Wrote silo.toml  (exe={exe_str}, proton={proton})");

    // ── 6. Re-archive with pigz into archive_dir ──────────────────────────────
    std::fs::create_dir_all(&cfg.archive_dir)?;
    let out_archive = cfg.archive_dir.join(format!("{game_name}.tar.gz"));
    println!("Archiving to {}...", out_archive.display());
    let status = Command::new("tar")
        .args([
            "--use-compress-program=pigz",
            "-cf",
            out_archive.to_str().context("non-UTF8 path")?,
            "-C",
            root_dir.to_str().context("non-UTF8 path")?,
            ".",
        ])
        .status()
        .context("failed to run tar — is pigz installed?")?;
    anyhow::ensure!(status.success(), "tar exited with {:?}", status.code());

    let size = std::fs::metadata(&out_archive)?.len();
    println!("Done: {game_name}.tar.gz ({:.2} GB)", size as f64 / 1_073_741_824.0);
    println!("Register it in silo as \"{game_name}\".");
    Ok(())
}

// ── Archive extraction ────────────────────────────────────────────────────────

fn extract_archive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = std::fs::File::open(src)?;
        tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(dest)?;
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        let file = std::fs::File::open(src)?;
        tar::Archive::new(xz2::read::XzDecoder::new(file)).unpack(dest)?;
    } else if name.ends_with(".zip") {
        zip::ZipArchive::new(std::fs::File::open(src)?)?.extract(dest)?;
    } else if name.ends_with(".rar") {
        extract_via_cmd("unrar", &["x", "-y", src.to_str().unwrap_or(""), dest.to_str().unwrap_or("")])
            .or_else(|_| extract_via_cmd("unar", &["-o", dest.to_str().unwrap_or(""), src.to_str().unwrap_or("")]))
            .context("rar extraction failed — install unrar or unar")?;
    } else if name.ends_with(".7z") {
        extract_via_cmd("7z", &["x", &format!("-o{}", dest.to_str().unwrap_or("")), "-y", src.to_str().unwrap_or("")])
            .or_else(|_| extract_via_cmd("7za", &["x", &format!("-o{}", dest.to_str().unwrap_or("")), "-y", src.to_str().unwrap_or("")]))
            .context("7z extraction failed — install p7zip")?;
    } else {
        extract_via_cmd("unar", &["-o", dest.to_str().unwrap_or(""), src.to_str().unwrap_or("")])
            .context(format!("unsupported archive format: {name}"))?;
    }

    Ok(())
}

fn extract_via_cmd(bin: &str, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new(bin)
        .args(args)
        .status()
        .with_context(|| format!("{bin} not found"))?;
    anyhow::ensure!(status.success(), "{bin} exited with {:?}", status.code());
    Ok(())
}

// ── Root directory detection ──────────────────────────────────────────────────

fn find_root(extract_dir: &Path) -> PathBuf {
    let entries: Vec<_> = std::fs::read_dir(extract_dir)
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        return entries[0].path();
    }

    extract_dir.to_path_buf()
}

// ── Exe detection ─────────────────────────────────────────────────────────────

const BAD_EXE_PATTERNS: &[&str] = &[
    "setup", "install", "unins", "crash", "redist", "vcredist", "vc_redist",
    "dotnet", "dxsetup", "oalinst", "physx", "directx", "prereq",
    "unitycrashhhandler", "ue4prereq", "ue5prereq", "cefsharp",
    "dxwebsetup", "windowsdesktop",
];

const BAD_DIRS: &[&str] = &[
    "_redist", "redist", "redistributable", "utilities", "tools",
    "support", "jre", "jdk", "_commonredist",
];

fn find_game_exes(root: &Path, game_name: &str) -> Vec<PathBuf> {
    let game_lower: String = game_name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    let mut scored: Vec<(i32, PathBuf)> = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| e.path().extension() == Some(OsStr::new("exe")))
        .filter_map(|e| {
            let path = e.path().to_path_buf();
            let rel = path.strip_prefix(root).ok()?;

            if rel.components().any(|c| {
                let s = c.as_os_str().to_string_lossy().to_lowercase();
                BAD_DIRS.iter().any(|bad| s == *bad)
            }) {
                return None;
            }

            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if BAD_EXE_PATTERNS.iter().any(|p| stem.contains(p)) {
                return None;
            }

            Some((score_exe(rel, &stem, &game_lower), rel.to_path_buf()))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, p)| p).collect()
}

fn score_exe(rel: &Path, stem: &str, game_lower: &str) -> i32 {
    let mut score = 0i32;
    let depth = rel.components().count() as i32;
    score += (6 - depth).max(-10) * 10;
    let stem_clean: String = stem.chars().filter(|c| c.is_alphanumeric()).collect();
    if stem_clean == *game_lower {
        score += 100;
    } else if game_lower.contains(&stem_clean) || stem_clean.contains(game_lower) {
        score += 50;
    }
    score
}

// ── Game name derivation ──────────────────────────────────────────────────────

fn derive_game_name(archive_path: &Path) -> String {
    let filename = archive_path.file_name().and_then(|n| n.to_str()).unwrap_or("game");

    let stem = filename
        .strip_suffix(".tar.gz")
        .or_else(|| filename.strip_suffix(".tar.xz"))
        .or_else(|| filename.strip_suffix(".tar.bz2"))
        .unwrap_or_else(|| {
            Path::new(filename).file_stem().and_then(|s| s.to_str()).unwrap_or(filename)
        });

    let mut name = stem.to_string();
    for (open, close) in [('[', ']'), ('(', ')')] {
        loop {
            match (name.find(open), name.find(close)) {
                (Some(s), Some(e)) if e > s => {
                    name = format!("{}{}", &name[..s], &name[e + 1..]);
                }
                _ => break,
            }
        }
    }
    for tag in &[" REPACK", " repack", "-CODEX", "-PLAZA", "-SKIDROW", "-EMPRESS", "-GOG"] {
        name = name.replace(tag, "");
    }
    name.trim().to_string()
}

// ── Arg parsing ───────────────────────────────────────────────────────────────

fn parse_args(args: Vec<String>) -> anyhow::Result<PathBuf> {
    match args.into_iter().next() {
        Some(p) => Ok(PathBuf::from(p)),
        None => anyhow::bail!("usage: silo-server normalize <archive>"),
    }
}
