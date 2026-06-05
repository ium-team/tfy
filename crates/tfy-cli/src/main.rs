use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tfy_core::*;

#[derive(Parser)]
#[command(name = "tfy", about = "Token-efficient AI work interface")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Index {
        path: PathBuf,
    },
    Expand {
        path: PathBuf,
        scope: String,
        #[arg(long, default_value = "symbol")]
        compactness: String,
    },
    Full {
        path: PathBuf,
        scope: String,
    },
    Restore {
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    DecideContext {
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    Run {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value_t = 1_000_000)]
        max_output_bytes: usize,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Tool Gateway entrypoint for agent runtimes that proxy ordinary commands through TFY.
    ToolGateway {
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long, default_value_t = 1_000_000)]
        max_output_bytes: usize,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    Raw {
        raw_ref: String,
        #[arg(long, default_value = ".tfy/raw")]
        raw_dir: PathBuf,
        #[arg(long)]
        around: Option<String>,
        #[arg(long, default_value_t = 3)]
        context: usize,
    },
    Languages,
    EvalCode {
        path: PathBuf,
        scope: String,
        #[arg(long, default_value = "symbol")]
        compactness: String,
    },
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Index { path } => print_json(&index_path(path)?)?,
        Cmd::Expand {
            path,
            scope,
            compactness,
        } => print_json(&expand_scope(path, &scope, &compactness)?)?,
        Cmd::Full { path, scope } => print_json(&full_scope(path, &scope)?)?,
        Cmd::Restore { payload } => {
            let text = read_payload(payload)?;
            let p: RestorePayload = serde_json::from_str(&text)?;
            print_json(&restore_payload(p)?)?;
        }
        Cmd::DecideContext { payload } => {
            let text = read_payload(payload)?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            let diagnostics = v.get("diagnostics").and_then(|x| x.as_str()).unwrap_or("");
            let compact_code = v
                .get("compact_code")
                .or_else(|| v.get("code"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let known_symbols: std::collections::BTreeMap<String, String> = v
                .get("symbols")
                .or_else(|| v.get("known_symbols"))
                .cloned()
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or_default();
            print_json(&decide_context_need(
                compact_code,
                &known_symbols,
                diagnostics,
            ))?;
        }
        Cmd::Run {
            raw_dir,
            max_output_bytes,
            command,
        }
        | Cmd::ToolGateway {
            raw_dir,
            max_output_bytes,
            command,
        } => execute_tool_gateway(command, raw_dir, max_output_bytes)?,
        Cmd::Raw {
            raw_ref,
            raw_dir,
            around,
            context,
        } => print!(
            "{}",
            raw_output(raw_dir, &raw_ref, around.as_deref(), context)?
        ),
        Cmd::Languages => print_json(&serde_json::json!({"languages": supported_languages()}))?,
        Cmd::EvalCode {
            path,
            scope,
            compactness,
        } => {
            let exp = expand_scope(&path, &scope, &compactness)?;
            let full = full_scope(&path, &scope)?;
            print_json(
                &serde_json::json!({"scope":exp.scope,"evaluation":evaluate_code(&full.code,&exp.compact_code),"char_metrics":exp.metrics}),
            )?;
        }
    }
    Ok(())
}
fn execute_tool_gateway(
    mut command: Vec<String>,
    raw_dir: PathBuf,
    max_output_bytes: usize,
) -> Result<()> {
    if command.first().map(|s| s == "--").unwrap_or(false) {
        command.remove(0);
    }
    let summary = run_command(&command, None, raw_dir, max_output_bytes)?;
    print!("{}", summary.summary);
    std::process::exit(summary.exit_code);
}

fn read_payload(path: Option<PathBuf>) -> Result<String> {
    Ok(if let Some(p) = path {
        std::fs::read_to_string(p)?
    } else {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
        s
    })
}
fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}
