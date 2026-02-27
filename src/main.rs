use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "Organiza episódios por série/temporada", long_about = None)]
struct Args {
    #[arg(short, long, default_value = ".")]
    input: PathBuf,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    verbose: bool,
}

#[derive(Debug)]
struct Parsed {
    show: String,
    season: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure_input_exists(&args.input)?;

    let patterns = build_patterns()?;
    let mut moved = 0usize;
    let mut skipped = 0usize;
    let mut counts: BTreeMap<(String, u8), usize> = BTreeMap::new();

    for entry in WalkDir::new(&args.input)
        .max_depth(1)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match parse_show_and_season(file_name, &patterns) {
            Some(parsed) => {
                let key = (parsed.show.clone(), parsed.season);
                *counts.entry(key).or_insert(0) += 1;

                let target_dir = build_target_dir(&args.input, &parsed);
                if already_in_target(path, &target_dir) {
                    if args.verbose {
                        println!("Já está organizado: {}", path.display());
                    }
                    skipped += 1;
                    continue;
                }

                let target_path = unique_target_path(&target_dir, path.file_name().unwrap());

                if args.dry_run {
                    println!("Moveria: {} -> {}", path.display(), target_path.display());
                } else {
                    fs::create_dir_all(&target_dir)
                        .with_context(|| format!("Criando pasta {}", target_dir.display()))?;
                    fs::rename(path, &target_path).with_context(|| {
                        format!("Movendo {} -> {}", path.display(), target_path.display())
                    })?;
                    println!("Movido: {} -> {}", path.display(), target_path.display());
                }
                moved += 1;
            }
            None => {
                if args.verbose {
                    println!("Sem correspondência: {}", path.display());
                }
                skipped += 1;
            }
        }
    }

    println!("Concluído. Movidos: {}, Ignorados: {}", moved, skipped);
    print_counts(&counts);
    Ok(())
}

fn ensure_input_exists(input: &Path) -> Result<()> {
    if input.is_dir() {
        return Ok(());
    }
    Err(anyhow::anyhow!("Pasta não encontrada: {}", input.display()))
}

fn build_patterns() -> Result<Vec<Regex>> {
    let patterns = vec![
        // Nome.S02E03.ext ou Nome - S02E03
        r"(?i)^(?P<show>.+?)[ ._-]*S(?P<season>\d{1,2})E\d{1,3}",
        // Nome - 2x03
        r"(?i)^(?P<show>.+?)[ ._-]*(?P<season>\d{1,2})x\d{1,3}",
        // Nome Season 2 / Nome - Season 2
        r"(?i)^(?P<show>.+?)[ ._-]*Season[ ._-]*(?P<season>\d{1,2})",
        // Nome Temporada 2 / Nome - Temporada 2
        r"(?i)^(?P<show>.+?)[ ._-]*Temporada[ ._-]*(?P<season>\d{1,2})",
        // Nome - S2
        r"(?i)^(?P<show>.+?)[ ._-]*S(?P<season>\d{1,2})[ ._-]",
    ];

    patterns
        .into_iter()
        .map(|p| Regex::new(p).context("Regex inválida"))
        .collect()
}

fn parse_show_and_season(file_name: &str, patterns: &[Regex]) -> Option<Parsed> {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())?;

    for pattern in patterns {
        if let Some(caps) = pattern.captures(stem) {
            let show_raw = caps.name("show")?.as_str();
            let season_str = caps.name("season")?.as_str();
            let season: u8 = season_str.parse().ok()?;
            let show = clean_show_name(show_raw);
            if show.is_empty() {
                continue;
            }
            return Some(Parsed { show, season });
        }
    }
    None
}

fn clean_show_name(raw: &str) -> String {
    let replaced = raw
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ");
    replaced
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_target_dir(base: &Path, parsed: &Parsed) -> PathBuf {
    let mut dir = base.to_path_buf();
    dir.push(&parsed.show);
    dir.push(format!("Season {:02}", parsed.season));
    dir
}

fn already_in_target(current: &Path, target_dir: &Path) -> bool {
    current.parent().map(|p| p == target_dir).unwrap_or(false)
}

fn unique_target_path(target_dir: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let mut candidate = target_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let mut counter = 1;
    loop {
        let mut name = format!("{} ({counter})", stem);
        if !ext.is_empty() {
            name.push('.');
            name.push_str(ext);
        }
        candidate = target_dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn print_counts(counts: &BTreeMap<(String, u8), usize>) {
    if counts.is_empty() {
        println!("Nenhum episódio reconhecido.");
        return;
    }

    println!("Episódios por temporada:");
    for ((show, season), count) in counts {
        println!("- {} - Season {:02}: {}", show, season, count);
    }
}
