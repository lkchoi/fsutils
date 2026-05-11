use clap::Parser;

#[derive(Parser)]
#[command(name = "ssim", about = "Find near-duplicate images using perceptual hashing and SSIM")]
struct Cli {
    /// Files, directories, or glob patterns
    #[arg(required = true)]
    paths: Vec<String>,
    #[arg(long, default_value = "0.95")]
    threshold: f64,
    #[arg(long, default_value = "10")]
    hash_threshold: u32,
}

fn main() {
    let cli = Cli::parse();
    let code = ssim::run(&cli.paths, cli.threshold, cli.hash_threshold);
    if code != 0 { std::process::exit(1); }
}
