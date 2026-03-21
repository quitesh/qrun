use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to write the journal log file
    #[arg(short, long)]
    journal: PathBuf,

    /// Shell to use for execution (default: zsh on Unix, cmd on Windows)
    #[arg(short, long)]
    shell: Option<String>,

    /// Command to execute
    command: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.command.is_empty() {
        eprintln!("Error: No command specified");
        std::process::exit(1);
    }

    let exit_code = qrun::run(qrun::RunConfig {
        journal: args.journal.clone(),
        shell: args.shell,
        command: args.command,
    })?;

    println!("Journal written to: {}", args.journal.display());
    println!("You can view it with: journalctl --file={}", args.journal.display());

    std::process::exit(exit_code)
}
