use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ssim", about = "Find near-duplicate images using perceptual hashing and SSIM")]
struct Cli {
    directory: PathBuf,
    #[arg(long, default_value = "0.95")]
    threshold: f64,
    #[arg(long, default_value = "10")]
    hash_threshold: u32,
}

fn main() {
    let cli = Cli::parse();
    let code = ssim::run(&cli.directory, cli.threshold, cli.hash_threshold);
    if code != 0 { std::process::exit(1); }
}
