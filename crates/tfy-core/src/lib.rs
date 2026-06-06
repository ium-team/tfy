pub mod code;
pub mod context;
pub mod eval;
pub mod language;
pub mod protocol;
pub mod tool_feedback;

pub use code::{expand_scope, full_scope, index_path, restore_payload};
pub use context::decide_context_need;
pub use eval::evaluate_code;
pub use language::{supported_languages, LanguageKind};
pub use protocol::*;
pub use tool_feedback::{
    classify_command_family, raw_output, raw_output_bytes, run_command, summarize_command_output,
    summarize_command_output_with_policy, CommandSummary, RawStore, ToolPolicy,
};

#[cfg(test)]
mod tests;
