use agentlint_core::{OutputFormat, Validator, format_gnu, format_json, run};
use clap::Parser;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "agentlint", about = "Lint AI coding agent harness files")]
struct Cli {
    /// Files or directories to validate (defaults to current directory)
    paths: Vec<PathBuf>,

    /// Output format
    #[arg(long, value_name = "FORMAT", default_value = "gnu")]
    format: String,

    /// Always exit 0 (audit mode)
    #[arg(long)]
    exit_zero: bool,
}

fn main() {
    let cli = Cli::parse();

    let format = match cli.format.as_str() {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Gnu,
    };

    let roots: Vec<PathBuf> = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };

    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(agentlint_claude::ClaudeValidator),
        Box::new(agentlint_cursor::CursorValidator),
        Box::new(agentlint_codex::CodexValidator),
        Box::new(agentlint_opencode::OpenCodeValidator),
        Box::new(agentlint_gemini::GeminiValidator),
        Box::new(agentlint_pi::PiValidator),
    ];

    let result = run(&roots, &validators);
    let has_errors = result
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, agentlint_core::Severity::Error));

    if result.diagnostics.is_empty() {
        if matches!(format, OutputFormat::Gnu) {
            // silent on clean
        }
    } else {
        let output = match format {
            OutputFormat::Gnu => format_gnu(&result.diagnostics),
            OutputFormat::Json => format_json(&result.diagnostics),
        };
        println!("{output}");

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
