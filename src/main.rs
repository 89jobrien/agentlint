use agentlint_core::config::load_config;
use agentlint_core::{
    Difficulty, OutputFormat, RunConfig, Validator, format_gnu, format_json, format_pretty, run,
};
use clap::Parser;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "agentlint",
    about = "Lint AI coding agent harness files",
    version
)]
struct Cli {
    /// Files or directories to validate (defaults to current directory)
    paths: Vec<PathBuf>,

    /// Output format: pretty | gnu | json (default: pretty when TTY, gnu when piped)
    #[arg(long, value_name = "FORMAT")]
    format: Option<String>,

    /// Difficulty level: easy | hard | painful (overrides .agentlint.toml)
    #[arg(long, value_name = "LEVEL")]
    difficulty: Option<String>,

    /// Always exit 0 (audit mode)
    #[arg(long)]
    exit_zero: bool,
}

fn main() {
    let cli = Cli::parse();

    let is_tty = std::io::stdout().is_terminal();
    let format = match cli.format.as_deref() {
        Some("json") => OutputFormat::Json,
        Some("gnu") => OutputFormat::Gnu,
        Some("pretty") => OutputFormat::Pretty,
        None if is_tty => OutputFormat::Pretty,
        _ => OutputFormat::Gnu,
    };

    // Load base config from .agentlint.toml (if present).
    let mut config = match load_config(std::path::Path::new(".agentlint.toml")) {
        Ok(Some(c)) => c,
        Ok(None) => RunConfig::default(),
        Err(e) => {
            eprintln!("agentlint: {e}");
            process::exit(2);
        }
    };

    // CLI --difficulty always wins over config file.
    if let Some(s) = cli.difficulty.as_deref() {
        config.difficulty = match Difficulty::from_str(s) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("agentlint: {e}");
                process::exit(2);
            }
        };
    }

    let roots: Vec<PathBuf> = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };

    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(agentlint_claude::ClaudeValidator),
        Box::new(agentlint_cursor::CursorValidator),
        Box::new(agentlint_codex::CodexValidator),
        Box::new(agentlint_opencode::AgentsMarkdownValidator),
        Box::new(agentlint_opencode::OpenCodeJsonValidator),
        Box::new(agentlint_gemini::GeminiValidator),
        Box::new(agentlint_pi::PiValidator),
    ];

    let result = run(&roots, &validators, &config);
    let has_errors = result
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, agentlint_core::Severity::Error));

    if !result.diagnostics.is_empty() {
        let output = match format {
            OutputFormat::Gnu => format_gnu(&result.diagnostics),
            OutputFormat::Json => format_json(&result.diagnostics),
            OutputFormat::Pretty => format_pretty(&result.diagnostics, is_tty),
        };
        print!("{output}");

        if matches!(format, OutputFormat::Gnu) {
            let errors = result
                .diagnostics
                .iter()
                .filter(|d| matches!(d.severity, agentlint_core::Severity::Error))
                .count();
            let warnings = result
                .diagnostics
                .iter()
                .filter(|d| matches!(d.severity, agentlint_core::Severity::Warning))
                .count();
            match (errors, warnings) {
                (e, 0) => eprintln!("{e} error{}", if e == 1 { "" } else { "s" }),
                (0, w) => eprintln!("{w} warning{}", if w == 1 { "" } else { "s" }),
                (e, w) => eprintln!(
                    "{e} error{}, {w} warning{}",
                    if e == 1 { "" } else { "s" },
                    if w == 1 { "" } else { "s" },
                ),
            }
        }
    }

    if has_errors && !cli.exit_zero {
        process::exit(1);
    }
}
