use clap::Parser;
use ddup::{
    apply_config, compile_excludes, format_size, hash_file_cached, move_to_trash, hard_delete,
    prompt_delete, resolve_paths, select_keep_index, set_finder_tag,
    DeleteAction, HashAlgorithm, KeepStrategy,
};
use std::collections::HashMap;
use std::fs;

#[derive(Parser, Debug)]
#[command(name = "ddup", about = "Find and manage duplicate files")]
struct Cli {
    #[arg(required = true)]
    paths: Vec<String>,
    #[arg(short, long, value_enum, default_value_t = HashAlgorithm::Xxh3)]
    algorithm: HashAlgorithm,
    #[arg(short, long, default_value_t = true)]
    recursive: bool,
    #[arg(short = 'n', long)]
    dry_run: bool,
    #[arg(short, long)]
    verbose: bool,
    /// Tag files with their hash in Finder instead of finding duplicates
    #[arg(long)]
    tag: bool,
    #[arg(long)]
    delete: bool,
    #[arg(short, long, value_enum, default_value = "best")]
    keep: Option<KeepStrategy>,
    /// Skip confirmation, move to Trash
    #[arg(long, requires = "delete", conflicts_with = "hard")]
    yes: bool,
    /// Skip confirmation, permanently delete
    #[arg(long, requires = "delete", conflicts_with = "yes")]
    hard: bool,
    #[arg(long)]
    no_cache: bool,
    /// Use perceptual similarity (SSIM) instead of exact hash for duplicate detection
    #[arg(long, default_value_t = true)]
    ssim: bool,
    /// SSIM threshold for similarity, 0.0-1.0
    #[arg(long, default_value = "0.95")]
    threshold: f64,
    /// Max Hamming distance for perceptual hash pre-filter
    #[arg(long, default_value = "10")]
    hash_threshold: u32,
    #[arg(short, long = "exclude", value_name = "PATTERN")]
    exclude: Vec<String>,
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    apply_config(&mut args);
    let cli = Cli::parse_from(&args);
    let exclude = compile_excludes(&cli.exclude);
    let files = resolve_paths(&cli.paths, cli.recursive, &exclude);

    if files.is_empty() { eprintln!("No files found."); std::process::exit(1); }

    let mut errors = 0;

    if cli.tag {
        for file in &files {
            match hash_file_cached(file, &cli.algorithm, cli.no_cache) {
                Ok(hash) => {
                    if cli.dry_run || cli.verbose { println!("{hash}  {}", file.display()); }
                    if cli.dry_run { continue; }
                    if let Err(e) = set_finder_tag(file, &hash) {
                        eprintln!("Error setting attribute on {}: {e}", file.display()); errors += 1;
                    }
                }
                Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); errors += 1; }
            }
        }
    } else {
        let dup_groups: Vec<(String, Vec<std::path::PathBuf>)> = if cli.ssim {
            let ssim_groups = ddup::ssim_duplicate_groups(&files, cli.threshold, cli.hash_threshold, &cli.algorithm, cli.no_cache);
            ssim_groups.into_iter().enumerate()
                .map(|(i, group)| (format!("group {}", i + 1), group))
                .collect()
        } else {
            let mut hash_map: HashMap<String, Vec<std::path::PathBuf>> = HashMap::new();
            for file in &files {
                match hash_file_cached(file, &cli.algorithm, cli.no_cache) {
                    Ok(hash) => { hash_map.entry(hash).or_default().push(file.clone()); }
                    Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); errors += 1; }
                }
            }
            hash_map.into_iter().filter(|(_, paths)| paths.len() > 1).collect()
        };

        if dup_groups.is_empty() {
            // no output
        } else if cli.delete {
            let mut to_trash: Vec<std::path::PathBuf> = Vec::new();
            for (label, paths) in &dup_groups {
                println!("{label}");
                if let Some(ref strategy) = cli.keep {
                    let keep_idx = select_keep_index(paths, strategy);
                    for (i, path) in paths.iter().enumerate() {
                        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                        if i == keep_idx { println!("  [keep]   {} {}", format_size(size), path.display()); }
                        else { println!("  [delete] {} {}", format_size(size), path.display()); to_trash.push(path.to_path_buf()); }
                    }
                } else {
                    for (i, path) in paths.iter().enumerate() {
                        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                        println!("  [{}] {} {}", i + 1, format_size(size), path.display());
                    }
                    eprint!("Keep which file? [1]: ");
                    std::io::Write::flush(&mut std::io::stderr()).ok();
                    let input = ddup::read_tty_line();
                    let keep_idx = match input.trim().parse::<usize>() {
                        Ok(n) if n >= 1 && n <= paths.len() => n - 1,
                        Ok(_) | Err(_) if input.trim().is_empty() => 0,
                        _ => { eprintln!("Invalid selection, skipping group."); continue; }
                    };
                    for (i, path) in paths.iter().enumerate() {
                        if i != keep_idx { to_trash.push(path.to_path_buf()); }
                    }
                }
            }
            if to_trash.is_empty() {
                eprintln!("Nothing to delete.");
            } else {
                let total_size: u64 = to_trash.iter()
                    .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
                    .sum();
                let action = if cli.yes {
                    eprintln!("Trashing {} file(s) ({}).", to_trash.len(), format_size(total_size));
                    DeleteAction::Trash
                } else if cli.hard {
                    eprintln!("Hard deleting {} file(s) ({}).", to_trash.len(), format_size(total_size));
                    DeleteAction::HardDelete
                } else {
                    println!();
                    prompt_delete(&format!("Delete {} file(s) ({})? [y]Trash / [h]ard delete / [N]o:", to_trash.len(), format_size(total_size)))
                };
                match action {
                    DeleteAction::Trash => {
                        for path in &to_trash {
                            match move_to_trash(path) {
                                Ok(()) => { if cli.verbose { eprintln!("Trashed: {}", path.display()); } }
                                Err(e) => { eprintln!("Error trashing {}: {e}", path.display()); errors += 1; }
                            }
                        }
                        if errors == 0 { eprintln!("Moved {} file(s) to Trash.", to_trash.len()); }
                    }
                    DeleteAction::HardDelete => {
                        for path in &to_trash {
                            match hard_delete(path) {
                                Ok(()) => { if cli.verbose { eprintln!("Deleted: {}", path.display()); } }
                                Err(e) => { eprintln!("Error deleting {}: {e}", path.display()); errors += 1; }
                            }
                        }
                        if errors == 0 { eprintln!("Deleted {} file(s).", to_trash.len()); }
                    }
                    DeleteAction::Abort => { eprintln!("Aborted."); }
                }
            }
        } else {
            for (label, paths) in &dup_groups {
                println!("{label}");
                for path in paths {
                    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    println!("  {} {}", format_size(size), path.display());
                }
            }
        }
    }

    if errors > 0 { std::process::exit(1); }
}
