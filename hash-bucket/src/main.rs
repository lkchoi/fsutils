use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hash-bucket", about = "Sort hash-named files into prefix directories")]
struct Cli {
    /// Source directory containing files
    #[arg(default_value = ".")]
    src: PathBuf,

    /// Copy files instead of moving
    #[arg(long)]
    cp: bool,

    /// Create bucket directories under this path instead of in-place
    #[arg(long)]
    target: Option<PathBuf>,

    /// Prefix length for bucket names
    #[arg(long, default_value = "2")]
    length: usize,

    /// Dry run: show what would be done
    #[arg(short = 'n', long)]
    dry_run: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.length < 1 {
        eprintln!("error: --length must be >= 1");
        std::process::exit(1);
    }

    let src = cli.src.canonicalize().unwrap_or_else(|e| {
        eprintln!("error: {}: {e}", cli.src.display());
        std::process::exit(1);
    });

    let target = match cli.target {
        Some(t) => {
            std::fs::create_dir_all(&t).ok();
            t.canonicalize().unwrap_or_else(|e| {
                eprintln!("error: {}: {e}", t.display());
                std::process::exit(1);
            })
        }
        None => src.clone(),
    };

    let code = hash_bucket::run(&src, &target, cli.length, cli.cp, cli.dry_run);
    if code != 0 {
        std::process::exit(1);
    }
}
