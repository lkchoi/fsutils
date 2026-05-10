use clap::Parser;
use fix_ext::run;

#[derive(Parser)]
#[command(name = "fix-ext", about = "Fix file extensions based on MIME type")]
struct Cli {
    #[arg(required = true)]
    paths: Vec<String>,
    #[arg(short = 'n', long)]
    dry_run: bool,
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    let code = run(&cli.paths, cli.dry_run, cli.verbose);
    if code != 0 { std::process::exit(1); }
}
