use clap::{Parser, Subcommand};
use dss::{run_comment, run_write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dss", about = "Finder metadata utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write .DS_Store files to set Finder view preferences recursively
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
    /// Set Finder comment on files
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
        Command::Write { path, icon_size, arrange_by, show_preview, dry_run, clean } =>
            run_write(&path, icon_size, &arrange_by, show_preview, dry_run, clean),
        Command::Comment { comment, paths, recursive, exclude, dry_run } =>
            run_comment(&comment, &paths, recursive, &exclude, dry_run),
    };
    if code != 0 { std::process::exit(1); }
}
