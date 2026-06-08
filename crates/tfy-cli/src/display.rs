use crate::util::{print_json, read_payload};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use tfy_core::{
    restore_display_payload, restore_file_payload, restore_patch_payload, RestorePayload,
};

#[derive(Args, Clone)]
pub(crate) struct RestoreDisplayCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct RestoreFileCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    /// Optional file path to write canonical restored readable code.
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub(crate) struct RestorePatchCmd {
    #[arg(long)]
    pub payload: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

pub(crate) fn execute_restore_display(cmd: RestoreDisplayCmd) -> Result<()> {
    let text = read_payload(cmd.payload)?;
    let payload: RestorePayload = serde_json::from_str(&text)?;
    let response = restore_display_payload(payload)?;
    if cmd.json {
        print_json(&response)?;
    } else {
        print!("{}", response.display_code);
        if let Some(warning) = response.warning {
            eprintln!("tfy restore-display warning: {warning}");
        }
    }
    Ok(())
}

pub(crate) fn execute_restore_file(cmd: RestoreFileCmd) -> Result<()> {
    let text = read_payload(cmd.payload)?;
    let payload: RestorePayload = serde_json::from_str(&text)?;
    let response = restore_file_payload(payload)?;
    if let Some(path) = &cmd.output {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, &response.file_code)?;
    }
    if cmd.json {
        print_json(&response)?;
    } else {
        print!("{}", response.file_code);
    }
    Ok(())
}

pub(crate) fn execute_restore_patch(cmd: RestorePatchCmd) -> Result<()> {
    let text = read_payload(cmd.payload)?;
    let payload: RestorePayload = serde_json::from_str(&text)?;
    let response = restore_patch_payload(payload)?;
    if cmd.json {
        print_json(&response)?;
    } else {
        print!("{}", response.file_code);
    }
    Ok(())
}
