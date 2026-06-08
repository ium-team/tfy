use crate::util::{print_json, read_payload};
use anyhow::Result;
use clap::Args;
use tfy_core::{restore_display_payload, RestorePayload};

#[derive(Args, Clone)]
pub(crate) struct RestoreDisplayCmd {
    #[arg(long)]
    pub payload: Option<std::path::PathBuf>,
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
