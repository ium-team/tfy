use crate::protocol::{Confidence, FallbackAction, ParserMetadata};
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageKind {
    Python,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Rust,
    Go,
    CFamily,
    Generic,
}

impl LanguageKind {
    pub fn from_path(path: &std::path::Path) -> Self {
        match path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "py" => Self::Python,
            "js" | "mjs" | "cjs" => Self::JavaScript,
            "jsx" => Self::Jsx,
            "ts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "rs" => Self::Rust,
            "go" => Self::Go,
            "c" | "h" | "cc" | "cpp" | "hpp" | "java" | "cs" => Self::CFamily,
            _ => Self::Generic,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::Jsx => "jsx",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::CFamily => "c-family",
            Self::Generic => "generic",
        }
    }
    pub fn parser_name(self) -> String {
        format!("tree-sitter:{}", self.name())
    }
    pub fn tree_sitter(self) -> Option<Language> {
        match self {
            Self::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Self::JavaScript | Self::Jsx => Some(tree_sitter_javascript::LANGUAGE.into()),
            Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Self::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Self::Go => Some(tree_sitter_go::LANGUAGE.into()),
            _ => None,
        }
    }
    pub fn metadata(self, has_error: bool, experimental: bool) -> ParserMetadata {
        let (confidence, fallback_action, reason) = if self.tree_sitter().is_none() || experimental
        {
            (
                Confidence::Low,
                FallbackAction::Full,
                "unsupported or experimental language; full fallback required".to_string(),
            )
        } else if has_error {
            (
                Confidence::Low,
                FallbackAction::Full,
                "tree-sitter reported syntax errors; full context required and symbol compaction declined".to_string(),
            )
        } else {
            (
                Confidence::High,
                FallbackAction::Selected,
                "tree-sitter parse succeeded with production adapter".to_string(),
            )
        };
        ParserMetadata {
            parser: self.parser_name(),
            confidence,
            fallback_action,
            reason,
        }
    }
    pub fn scope_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["function_definition", "class_definition"],
            Self::JavaScript | Self::Jsx | Self::TypeScript | Self::Tsx => &[
                "function_declaration",
                "method_definition",
                "class_declaration",
                "arrow_function",
                "function",
            ],
            Self::Rust => &[
                "function_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "impl_item",
            ],
            Self::Go => &[
                "function_declaration",
                "method_declaration",
                "type_declaration",
            ],
            _ => &[],
        }
    }
}

pub fn supported_languages() -> Vec<&'static str> {
    vec![
        "python",
        "javascript",
        "jsx",
        "typescript",
        "tsx",
        "rust",
        "go",
        "c-family-experimental",
    ]
}
