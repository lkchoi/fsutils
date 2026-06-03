use clap::Parser;
use towebp::run;

#[derive(Parser)]
#[command(name = "towebp", about = "Convert images to WebP, preserving file attributes")]
struct Cli {
    #[arg(required = true)]
    paths: Vec<String>,
    #[arg(short = 'n', long)]
    dry_run: bool,
    #[arg(short, long)]
    verbose: bool,
    #[arg(short, long, default_value_t = 80)]
    quality: u8,
    /// Keep original file after conversion
    #[arg(short, long)]
    keep: bool,
}

fn main() {
    let cli = Cli::parse();
    let code = run(&cli.paths, cli.quality, cli.dry_run, cli.verbose, cli.keep);
    if code != 0 { std::process::exit(1); }
}
