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

    /// Suppress the summary stats table
    #[arg(long, short)]
    quiet: bool,

    /// Infer docs schema from corpus and validate outliers against it
    #[arg(long)]
    infer_schema: bool,

    /// Infer docs schema from corpus and print JSON to stdout (implies --infer-schema)
    #[arg(long)]
    emit_schema: bool,
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

    let infer = cli.infer_schema || cli.emit_schema;

    // Handle --emit-schema: collect docs files, infer schema, print JSON, exit.
    if cli.emit_schema {
        let mut docs_files = Vec::new();
        for root in &roots {
            collect_docs_files(root, &mut docs_files);
        }
        let corpus: Vec<(&std::path::Path, &str)> = docs_files
            .iter()
            .map(|(p, s)| (p.as_path(), s.as_str()))
            .collect();
        let inferred = agentlint_docs::infer_schema(&corpus);
        let json = serde_json::to_string_pretty(&inferred).unwrap_or_else(|_| "{}".to_string());
        println!("{json}");
        return;
    }

    let mut docs_validator = agentlint_docs::DocsValidator::new(
        agentlint_docs::DocsSchema::from_config_path(std::path::Path::new(".agentlint.toml"))
            .unwrap_or_default(),
    );
    docs_validator.set_infer_mode(infer);

    // Rust-native validators (behavioral rules not expressible declaratively).
    let mut validators: Vec<Box<dyn Validator>> = vec![
        Box::new(agentlint_claude::ClaudeValidator),
        Box::new(agentlint_cursor::CursorValidator),
        Box::new(agentlint_codex::CodexValidator),
        Box::new(agentlint_opencode::AgentsMarkdownValidator),
        Box::new(agentlint_opencode::OpenCodeJsonValidator),
        Box::new(agentlint_gemini::GeminiValidator),
        Box::new(agentlint_pi::PiValidator),
        Box::new(agentlint_looprs::CommandsValidator),
        Box::new(agentlint_looprs::HooksValidator),
        Box::new(agentlint_looprs::SkillsValidator),
        Box::new(agentlint_looprs::AgentsValidator),
        Box::new(agentlint_looprs::RulesValidator),
        Box::new(agentlint_looprs::ConfigValidator),
        Box::new(agentlint_looprs::AgentJsonValidator),
        Box::new(docs_validator),
    ];

    // Declarative validators from embedded plugin TOMLs.
    validators.extend(agentlint_plugins::all_validators());

    // External plugins from plugins/ directory (user-defined).
    let (ext_validators, ext_errors) =
        agentlint_plugins::load_external(std::path::Path::new("plugins"));
    for e in ext_errors {
        eprintln!("agentlint: {e}");
    }
    validators.extend(ext_validators);

    let result = run(&roots, &validators, &config);
    let has_errors = result
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, agentlint_core::Severity::Error));

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

    if !result.diagnostics.is_empty() {
        let output = match format {
            OutputFormat::Gnu => format_gnu(&result.diagnostics),
            OutputFormat::Json => format_json(&result.diagnostics),
            OutputFormat::Pretty => format_pretty(&result.diagnostics, is_tty),
        };
        print!("{output}");

        if matches!(format, OutputFormat::Gnu) {
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

    // Summary stats table (shown by default, suppressed with --quiet).
    if !cli.quiet {
        let files = result.files_checked;
        let validator_count = validators.len();
        eprintln!();
        eprintln!("  Files checked : {files}");
        for (cat, count) in &result.file_counts {
            eprintln!("    {cat:<14}: {count}");
        }
        eprintln!("  Validators    : {validator_count}");
        eprintln!("  Errors        : {errors}");
        eprintln!("  Warnings      : {warnings}");
        if errors == 0 && warnings == 0 {
            eprintln!("  Result        : all clean");
        }
    }

    if has_errors && !cli.exit_zero {
        process::exit(1);
    }
}

/// Walk `root` and collect all `docs/**/*.md` files (path + content).
fn collect_docs_files(root: &std::path::Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.into_path();
        let is_docs_md = path.components().any(|c| c.as_os_str() == "docs")
            && path.extension().and_then(|e| e.to_str()) == Some("md");
        if is_docs_md && let Ok(src) = std::fs::read_to_string(&path) {
            out.push((path, src));
        }
    }
}
