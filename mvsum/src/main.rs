use clap::Parser;
use mvsum::{Algorithm, run};

#[derive(Parser)]
#[command(name = "mvsum", about = "Rename files to their hash")]
struct Cli {
    #[arg(required = true)]
    paths: Vec<String>,

    #[arg(short, long, value_enum, default_value_t = Algorithm::Md5)]
    algorithm: Algorithm,

    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    let code = run(&cli.paths, &cli.algorithm, cli.verbose);
    if code != 0 { std::process::exit(1); }
}
