use anyhow::{bail, Result};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tfy_runtime::{GatewayKind, OutputMode};

pub(crate) fn read_payload(path: Option<PathBuf>) -> Result<String> {
    Ok(if let Some(p) = path {
        std::fs::read_to_string(p)?
    } else {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
        s
    })
}
pub(crate) fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

pub(crate) fn parse_gateway(value: &str) -> Result<GatewayKind> {
    match value.to_ascii_lowercase().as_str() {
        "tool" => Ok(GatewayKind::Tool),
        "context" => Ok(GatewayKind::Context),
        "output" => Ok(GatewayKind::Output),
        "state" => Ok(GatewayKind::State),
        _ => bail!("unknown gateway: {value}"),
    }
}

pub(crate) fn parse_output_mode(value: &str) -> Result<OutputMode> {
    match value.to_ascii_lowercase().as_str() {
        "text" => Ok(OutputMode::Text),
        "json" => Ok(OutputMode::Json),
        "jsonl" => Ok(OutputMode::Jsonl),
        "mcp_resource" | "mcp-resource" => Ok(OutputMode::McpResource),
        "provider_payload" | "provider-payload" => Ok(OutputMode::ProviderPayload),
        _ => bail!("unknown output mode: {value}"),
    }
}

pub(crate) fn stable_id(input: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("tfy_{:016x}", hasher.finish())
}
