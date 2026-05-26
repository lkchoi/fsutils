use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "fsutils", about = "File utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Rename files to their hash
    Mvsum {
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(short, long, value_enum, default_value_t = mvsum::Algorithm::Xxh3)]
        algorithm: mvsum::Algorithm,
        #[arg(short, long)]
        verbose: bool,
    },
    /// Find and manage duplicate files
    Ddup {
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(short, long, value_enum, default_value_t = ddup::HashAlgorithm::Xxh3)]
        algorithm: ddup::HashAlgorithm,
        #[arg(short, long, default_value_t = true)]
        recursive: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
        #[arg(long)]
        delete: bool,
        #[arg(short, long, value_enum, default_value = "best")]
        keep: Option<ddup::KeepStrategy>,
        /// Skip confirmation, move to Trash
        #[arg(long, requires = "delete", conflicts_with = "hard")]
        yes: bool,
        /// Skip confirmation, permanently delete
        #[arg(long, requires = "delete", conflicts_with = "yes")]
        hard: bool,
        #[arg(long)]
        no_cache: bool,
        #[arg(short, long = "exclude", value_name = "PATTERN")]
        exclude: Vec<String>,
        /// Use perceptual similarity (SSIM) instead of exact hash for duplicate detection
        #[arg(long, default_value_t = true)]
        ssim: bool,
        /// SSIM threshold for similarity, 0.0-1.0
        #[arg(long, default_value = "0.95")]
        threshold: f64,
        /// Max Hamming distance for perceptual hash pre-filter
        #[arg(long, default_value = "10")]
        hash_threshold: u32,
    },
    /// Finder metadata utilities
    Dss {
        #[command(subcommand)]
        command: DssCommand,
    },
    /// Find near-duplicate images using perceptual hashing and SSIM
    Ssim {
        /// Files, directories, or glob patterns
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(long, default_value = "0.95")]
        threshold: f64,
        #[arg(long, default_value = "10")]
        hash_threshold: u32,
    },
    /// Fix file extensions based on MIME type
    FixExt {
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(short = 'n', long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum DssCommand {
    Write {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "128")]
        icon_size: u16,
        #[arg(long, default_value = "size")]
        arrange_by: String,
        #[arg(long, default_value = "true")]
        show_preview: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        clean: bool,
    },
    Comment {
        comment: String,
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(short, long, default_value_t = true)]
        recursive: bool,
        #[arg(short, long = "exclude", value_name = "PATTERN")]
        exclude: Vec<String>,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Mvsum { paths, algorithm, verbose } =>
            mvsum::run(&paths, &algorithm, verbose),
        Command::Ddup { paths, algorithm, recursive, dry_run, verbose: _, delete, keep, yes, hard, no_cache, exclude, ssim, threshold, hash_threshold } => {
            use ddup::{compile_excludes, format_size, hash_file_cached, move_to_trash, hard_delete as do_hard_delete, prompt_delete, resolve_paths, select_keep_index, set_finder_tag, ssim_duplicate_groups, DeleteAction};
            use std::collections::HashMap;
            use std::fs;

            let excl = compile_excludes(&exclude);
            let files = resolve_paths(&paths, recursive, &excl);
            if files.is_empty() { eprintln!("No files found."); std::process::exit(1); }

            let mut errors = 0i32;

            let dup_groups: Vec<(String, Vec<PathBuf>)> = if ssim {
                let ssim_groups = ssim_duplicate_groups(&files, threshold, hash_threshold, &algorithm, no_cache);
                if !dry_run {
                    for file in &files {
                        match hash_file_cached(file, &algorithm, no_cache) {
                            Ok(hash) => {
                                if let Err(e) = set_finder_tag(file, &hash) {
                                    eprintln!("Error setting attribute on {}: {e}", file.display()); errors += 1;
                                }
                            }
                            Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); errors += 1; }
                        }
                    }
                }
                ssim_groups.into_iter().enumerate()
                    .map(|(i, group)| (format!("group {}", i + 1), group))
                    .collect()
            } else {
                let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
                for file in &files {
                    match hash_file_cached(file, &algorithm, no_cache) {
                        Ok(hash) => {
                            if !dry_run {
                                if let Err(e) = set_finder_tag(file, &hash) {
                                    eprintln!("Error setting attribute on {}: {e}", file.display()); errors += 1;
                                }
                            }
                            hash_map.entry(hash).or_default().push(file.clone());
                        }
                        Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); errors += 1; }
                    }
                }
                hash_map.into_iter().filter(|(_, p)| p.len() > 1).collect()
            };

            if dup_groups.is_empty() {
                // no output
            } else if delete {
                let mut to_trash: Vec<PathBuf> = Vec::new();
                for (label, paths) in &dup_groups {
                    println!("{label}");
                    if let Some(ref strategy) = keep {
                        let keep_idx = select_keep_index(paths, strategy);
                        for (i, path) in paths.iter().enumerate() {
                            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                            if i == keep_idx { println!("  [keep]   {} {}", format_size(size), path.display()); }
                            else { println!("  [delete] {} {}", format_size(size), path.display()); to_trash.push(path.to_path_buf()); }
                        }
                    }
                }
                if to_trash.is_empty() {
                    eprintln!("Nothing to delete.");
                } else {
                    let total_size: u64 = to_trash.iter()
                        .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
                        .sum();
                    let action = if yes {
                        eprintln!("Trashing {} file(s) ({}).", to_trash.len(), format_size(total_size));
                        DeleteAction::Trash
                    } else if hard {
                        eprintln!("Hard deleting {} file(s) ({}).", to_trash.len(), format_size(total_size));
                        DeleteAction::HardDelete
                    } else {
                        println!();
                        prompt_delete(&format!("Delete {} file(s) ({})? [y]Trash / [h]ard delete / [N]o:", to_trash.len(), format_size(total_size)))
                    };
                    match action {
                        DeleteAction::Trash => {
                            for path in &to_trash {
                                if let Err(e) = move_to_trash(path) { eprintln!("Error trashing {}: {e}", path.display()); errors += 1; }
                            }
                            if errors == 0 { eprintln!("Moved {} file(s) to Trash.", to_trash.len()); }
                        }
                        DeleteAction::HardDelete => {
                            for path in &to_trash {
                                if let Err(e) = do_hard_delete(path) { eprintln!("Error deleting {}: {e}", path.display()); errors += 1; }
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
            errors
        }
        Command::Dss { command } => match command {
            DssCommand::Write { path, icon_size, arrange_by, show_preview, dry_run, clean } =>
                dss::run_write(&path, icon_size, &arrange_by, show_preview, dry_run, clean),
            DssCommand::Comment { comment, paths, recursive, exclude, dry_run } =>
                dss::run_comment(&comment, &paths, recursive, &exclude, dry_run),
        },
        Command::Ssim { paths, threshold, hash_threshold } =>
            ssim::run(&paths, threshold, hash_threshold),
        Command::FixExt { paths, dry_run, verbose } =>
            fix_ext::run(&paths, dry_run, verbose),
    };
    if code != 0 { std::process::exit(1); }
}
