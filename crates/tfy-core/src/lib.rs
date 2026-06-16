pub mod code;
pub mod context;
pub mod eval;
pub mod language;
pub mod protocol;
pub mod tool_feedback;

pub use code::{
    apply_restored_payload, expand_scope, full_scope, index_path, restore_display_payload,
    restore_file_payload, restore_patch_payload, restore_payload,
};
pub use context::decide_context_need;
pub use eval::evaluate_code;
pub use language::{supported_languages, LanguageKind};
pub use protocol::*;
pub use tool_feedback::{
    classify_command_family, classify_command_strategy, command_strategy_metadata, raw_output,
    raw_output_bytes, run_command, summarize_command_output, summarize_command_output_with_policy,
    CommandStrategyMetadata, CommandStrategyRegistry, CommandSummary, RawStore, ToolPolicy,
};

#[cfg(test)]
mod tests;
