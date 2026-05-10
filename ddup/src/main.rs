use clap::Parser;
use ddup::{
    apply_config, compile_excludes, format_size, hash_file_cached, move_to_trash,
    prompt_yn, resolve_paths, select_keep_index, set_finder_tag, HashAlgorithm, KeepStrategy,
};
use std::collections::HashMap;
use std::fs;

#[derive(Parser, Debug)]
#[command(name = "ddup", about = "Set file hash sums as Finder comments or tags")]
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
    #[arg(short, long)]
    duplicates: bool,
    #[arg(long, requires = "duplicates")]
    delete: bool,
    #[arg(short, long, value_enum, requires = "delete")]
    keep: Option<KeepStrategy>,
    #[arg(long)]
    no_cache: bool,
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

    if cli.duplicates {
        let mut hash_map: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
        for file in &files {
            match hash_file_cached(file, &cli.algorithm, cli.no_cache) {
                Ok(hash) => { hash_map.entry(hash).or_default().push(file); }
                Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); errors += 1; }
            }
        }

        let dup_groups: Vec<_> = hash_map.into_iter().filter(|(_, paths)| paths.len() > 1).collect();

        if dup_groups.is_empty() {
            eprintln!("No duplicates found.");
        } else if cli.delete {
            let mut to_trash: Vec<std::path::PathBuf> = Vec::new();
            for (hash, paths) in &dup_groups {
                println!("{hash}");
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
                    use std::io::BufRead as _;
                    let mut input = String::new();
                    std::io::stdin().lock().read_line(&mut input).ok();
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
                println!();
                if prompt_yn(&format!("Move {} file(s) to Trash? [y/N]:", to_trash.len())) {
                    for path in &to_trash {
                        match move_to_trash(path) {
                            Ok(()) => { if cli.verbose { eprintln!("Trashed: {}", path.display()); } }
                            Err(e) => { eprintln!("Error trashing {}: {e}", path.display()); errors += 1; }
                        }
                    }
                    if errors == 0 { eprintln!("Moved {} file(s) to Trash.", to_trash.len()); }
                } else { eprintln!("Aborted."); }
            }
        } else {
            for (hash, paths) in &dup_groups {
                println!("{hash}");
                for path in paths {
                    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    println!("  {} {}", format_size(size), path.display());
                }
            }
        }
    } else {
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
    }

    if errors > 0 { std::process::exit(1); }
}
