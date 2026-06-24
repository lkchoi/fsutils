use clap::Parser;

#[derive(Parser)]
#[command(name = "ssim dupes", about = "Find duplicate groups among images")]
struct DupesCli {
    /// Files, directories, or glob patterns
    #[arg(required = true)]
    paths: Vec<String>,
    #[arg(long, default_value = "0.95")]
    threshold: f64,
    #[arg(long, default_value = "10")]
    hash_threshold: u32,
}

#[derive(Parser)]
#[command(name = "ssim", about = "Find images similar to a reference image (reverse image search)\n\nUsage: ssim <IMAGE> [DIRS...]\n       ssim dupes <PATHS...>")]
struct SearchCli {
    /// Reference image to search for
    image: String,
    /// Directories to search
    #[arg(default_value = ".")]
    dirs: Vec<String>,
    #[arg(long, default_value = "0.80")]
    threshold: f64,
    #[arg(long, default_value = "10")]
    hash_threshold: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let code = if args.get(1).is_some_and(|a| a == "dupes") {
        // Strip "dupes" and re-parse as DupesCli
        let filtered: Vec<String> = std::iter::once(args[0].clone())
            .chain(args[2..].iter().cloned())
            .collect();
        let cli = DupesCli::parse_from(filtered);
        ssim::run(&cli.paths, cli.threshold, cli.hash_threshold)
    } else {
        let cli = SearchCli::parse();
        match ssim::find_similar(&cli.image, &cli.dirs, cli.threshold, cli.hash_threshold) {
            Ok(matches) => {
                if matches.is_empty() {
                    eprintln!("No similar images found.");
                } else {
                    for m in &matches {
                        println!("{:.4} {}", m.score, m.path.display());
                    }
                }
                0
            }
            Err(e) => { eprintln!("Error: {}", e); 1 }
        }
    };
    if code != 0 { std::process::exit(1); }
}
