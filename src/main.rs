mod analysis;
mod parser;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use analysis::{CommandFrequency, CommandSequence, ErrorDetector, TimePatterns, WorkflowDetector};

#[derive(Parser)]
#[command(name = "shell-history-analyzer")]
#[command(about = "Analyze your shell history to discover patterns, workflows, and habits")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to history file (auto-detected if not specified)
    #[arg(short, long, global = true)]
    file: Option<PathBuf>,

    /// Number of results to show
    #[arg(short, long, default_value = "20")]
    top: usize,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show top-N most used commands
    Frequency,
    /// Show command sequences (what follows what?)
    Sequence {
        /// Show transitions for a specific command
        #[arg(short, long)]
        command: Option<String>,
    },
    /// Show time-of-day patterns
    Time,
    /// Detect likely failed commands
    Errors,
    /// Detect common workflows
    Workflows,
    /// Full summary report
    Report,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let history_path = cli.file.clone().or_else(parser::detect_history_file);
    let path = history_path.ok_or_else(|| anyhow::anyhow!("No history file found. Specify --file"))?;

    let entries = parser::parse_history(&path)?;

    match cli.command {
        Commands::Frequency => {
            let freq = CommandFrequency::analyze(&entries, cli.top);
            freq.print(cli.json);
        }
        Commands::Sequence { command } => {
            let seq = CommandSequence::analyze(&entries, cli.top);
            if let Some(cmd) = command {
                seq.print_for_command(&cmd, cli.json);
            } else {
                seq.print(cli.json);
            }
        }
        Commands::Time => {
            let time = TimePatterns::analyze(&entries);
            time.print(cli.json);
        }
        Commands::Errors => {
            let errors = ErrorDetector::analyze(&entries, cli.top);
            errors.print(cli.json);
        }
        Commands::Workflows => {
            let workflows = WorkflowDetector::analyze(&entries);
            workflows.print(cli.json);
        }
        Commands::Report => {
            println!("📊 Shell History Analysis Report");
            println!("═══════════════════════════════════════");
            println!("📁 File: {}", path.display());
            println!("📝 Total entries: {}\n", entries.len());

            let freq = CommandFrequency::analyze(&entries, cli.top);
            println!("── Top Commands ──");
            freq.print(false);

            let seq = CommandSequence::analyze(&entries, 5);
            println!("\n── Command Sequences ──");
            seq.print(false);

            let time = TimePatterns::analyze(&entries);
            println!("\n── Time Patterns ──");
            time.print(false);

            let errors = ErrorDetector::analyze(&entries, cli.top);
            println!("\n── Likely Errors ──");
            errors.print(false);

            let workflows = WorkflowDetector::analyze(&entries);
            println!("\n── Workflows ──");
            workflows.print(false);
        }
    }

    Ok(())
}
