use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSummary {
    pub command: String,
    pub exit_code: i32,
    pub risk: String,
    pub summary: String,
    pub model_text: String,
    pub rendering_kind: String,
    pub raw_ref: String,
    pub raw_chars: usize,
    pub summary_chars: usize,
    pub savings_pct: f64,
    pub evidence: Vec<String>,
    pub command_family: String,
    pub strategy_kind: String,
    pub human_auto_safe: bool,
    pub agent_safe: bool,
    pub interactive_risk: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub strategy_source_kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_rule_diagnostics: Vec<CommandRuleDiagnostic>,
    pub output_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRuleDiagnostic {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandStrategyMetadata {
    pub family: String,
    pub strategy_kind: String,
    pub human_auto_safe: bool,
    pub agent_safe: bool,
    pub streaming_safe: bool,
    pub interactive_risk: String,
    pub claim_status: String,
}

#[derive(Debug, Clone, Copy)]
struct CommandStrategySpec {
    family: &'static str,
    strategy_kind: &'static str,
    human_auto_safe: bool,
    agent_safe: bool,
    streaming_safe: bool,
    interactive_risk: &'static str,
    claim_status: &'static str,
}

const USER_RULE_PUBLIC_SCAN_LINE_CHARS: usize = 1_000;
const USER_RULE_STRUCTURED_PARSE_MAX_BYTES: usize = 1_000_000;
const USER_RULE_STRUCTURED_MAX_ITEMS: usize = 100;
const USER_RULE_GROUP_MAX_DISTINCT: usize = 100;

const RUST_STRATEGY_FAMILIES: &[&str] = &[
    "git_status",
    "git_diff",
    "git_log",
    "gh_pr_checks",
    "cargo_test",
    "cargo_clippy",
    "cargo_build",
    "cargo_check",
    "cargo_fmt_check",
    "tsc_check",
    "pytest",
    "npm_test",
    "pnpm_test",
    "yarn_test",
    "go_test",
    "maven_test",
    "gradle_test",
];

const GENERIC_STRATEGY_SPEC: CommandStrategySpec = CommandStrategySpec {
    family: "generic",
    strategy_kind: "generic",
    human_auto_safe: false,
    agent_safe: true,
    streaming_safe: false,
    interactive_risk: "unknown",
    claim_status: "generic_fallback",
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandStrategyRegistry;

impl CommandStrategyRegistry {
    pub fn classify(&self, command: &str) -> CommandStrategyMetadata {
        self.metadata_for_family(&classify_command_family(command))
    }

    pub fn metadata_for_family(&self, family: &str) -> CommandStrategyMetadata {
        if RUST_STRATEGY_FAMILIES.contains(&family) {
            return CommandStrategyMetadata {
                family: family.to_string(),
                strategy_kind: "rust".to_string(),
                human_auto_safe: false,
                agent_safe: true,
                streaming_safe: false,
                interactive_risk: "none".to_string(),
                claim_status: "implemented_p0_not_comparison_claimed".to_string(),
            };
        }
        if let Some(filter) = built_in_filter_for_family(family) {
            return CommandStrategyMetadata {
                family: filter.family.to_string(),
                strategy_kind: "dsl".to_string(),
                human_auto_safe: filter.human_auto_safe,
                agent_safe: true,
                streaming_safe: false,
                interactive_risk: filter.interactive_risk.to_string(),
                claim_status: "built_in_filter_foundation".to_string(),
            };
        }
        CommandStrategyMetadata {
            family: GENERIC_STRATEGY_SPEC.family.to_string(),
            strategy_kind: GENERIC_STRATEGY_SPEC.strategy_kind.to_string(),
            human_auto_safe: GENERIC_STRATEGY_SPEC.human_auto_safe,
            agent_safe: GENERIC_STRATEGY_SPEC.agent_safe,
            streaming_safe: GENERIC_STRATEGY_SPEC.streaming_safe,
            interactive_risk: GENERIC_STRATEGY_SPEC.interactive_risk.to_string(),
            claim_status: GENERIC_STRATEGY_SPEC.claim_status.to_string(),
        }
    }

    fn summary_candidate(&self, input: StrategySummaryInput<'_>) -> Option<String> {
        rust_strategy_summary_candidate(
            input.family,
            input.cmd,
            input.code,
            input.raw,
            input.evidence,
            input.rr,
            input.risk,
        )
        .or_else(|| {
            built_in_filter_summary_candidate(
                input.family,
                input.cmd,
                input.code,
                input.raw,
                input.evidence,
                input.rr,
                input.risk,
            )
        })
    }
}

struct StrategySummaryInput<'a> {
    family: &'a str,
    cmd: &'a str,
    code: i32,
    raw: &'a str,
    evidence: &'a [String],
    rr: &'a str,
    risk: &'a str,
}

pub fn command_strategy_metadata(family: &str) -> CommandStrategyMetadata {
    CommandStrategyRegistry.metadata_for_family(family)
}

pub fn classify_command_strategy(command: &str) -> CommandStrategyMetadata {
    CommandStrategyRegistry.classify(command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    Auto,
    Generic,
    GitGithub,
}

#[derive(Debug, Clone, Default)]
pub struct CommandRuleSet {
    rules: Vec<CommandRule>,
    diagnostics: Vec<CommandRuleDiagnostic>,
}

#[derive(Debug, Clone)]
struct CommandRule {
    id: String,
    source_kind: String,
    argv_prefix: Vec<String>,
    command_regex: Option<Regex>,
    safety: RuleSafetyMetadata,
    override_policy: RuleOverridePolicy,
    operations: Vec<RuleOperation>,
}

#[derive(Debug, Clone)]
struct RuleSafetyMetadata {
    human_auto_safe: bool,
    agent_safe: bool,
    interactive_risk: String,
}

#[derive(Debug, Clone, Default)]
struct RuleOverridePolicy {
    built_in: bool,
    family: Option<String>,
    reason: Option<String>,
}

impl RuleOverridePolicy {
    fn allows_family(&self, family: &str) -> bool {
        self.built_in && self.family.as_deref() == Some(family)
    }
}

#[derive(Debug, Clone)]
enum RuleOperation {
    LineFilter(RuntimeFilter),
    Section(SectionSpec),
    Counter(CounterSpec),
    Capture(CaptureSpec),
    Severity(SeveritySpec),
    StructuredExtract(StructuredExtractSpec),
    Metric(MetricSpec),
    Group(GroupSpec),
}

#[derive(Debug, Clone)]
struct SectionSpec {
    title: String,
    filter: RuntimeFilter,
    empty: SectionEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionEmpty {
    Omit,
    Show,
}

#[derive(Debug, Clone)]
struct CounterSpec {
    name: String,
    pattern: Regex,
    max_count: usize,
}

#[derive(Debug, Clone)]
struct CaptureSpec {
    name: String,
    pattern: Regex,
    field: String,
    dedupe: bool,
    max_items: usize,
}

#[derive(Debug, Clone)]
struct SeveritySpec {
    level: SeverityLevel,
    pattern: Regex,
}

#[derive(Debug, Clone)]
enum StructuredExtractSpec {
    Json(JsonExtractSpec),
    Ndjson(JsonExtractSpec),
    Kv(KvExtractSpec),
    Table(TableExtractSpec),
}

#[derive(Debug, Clone)]
struct JsonExtractSpec {
    name: String,
    path: String,
    max_items: usize,
}

#[derive(Debug, Clone)]
struct KvExtractSpec {
    name: String,
    key: String,
    separators: Vec<char>,
    max_items: usize,
}

#[derive(Debug, Clone)]
struct TableExtractSpec {
    name: String,
    columns: Vec<String>,
    delimiter: TableDelimiter,
    max_rows: usize,
}

#[derive(Debug, Clone, Copy)]
enum TableDelimiter {
    Whitespace,
    Comma,
}

#[derive(Debug, Clone)]
struct MetricSpec {
    name: String,
    op: MetricOp,
    pattern: Regex,
    max_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum MetricOp {
    Count,
    UniqueCount,
}

#[derive(Debug, Clone)]
struct GroupSpec {
    name: String,
    pattern: Regex,
    field: String,
    top_k: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SeverityLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl SeverityLevel {
    fn as_str(self) -> &'static str {
        match self {
            SeverityLevel::Info => "info",
            SeverityLevel::Warning => "warning",
            SeverityLevel::Error => "error",
            SeverityLevel::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeFilter {
    strip_ansi: bool,
    strip_lines_matching: Vec<Regex>,
    keep_lines_matching: Vec<Regex>,
    preserve_lines_matching: Vec<Regex>,
    truncate_lines_at: usize,
    head_lines: usize,
    tail_lines: usize,
    max_lines: usize,
    on_empty: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRulesFile {
    #[serde(default)]
    schema_version: Option<u16>,
    #[serde(default)]
    command: Vec<CommandRuleToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleToml {
    id: String,
    #[serde(default, rename = "description")]
    _description: Option<String>,
    #[serde(default, rename = "match")]
    match_config: CommandRuleMatchToml,
    #[serde(default = "default_true")]
    strip_ansi: bool,
    #[serde(default)]
    preserve_lines_matching: Vec<String>,
    #[serde(default)]
    strip_lines_matching: Vec<String>,
    #[serde(default)]
    keep_lines_matching: Vec<String>,
    #[serde(default = "default_truncate_lines_at")]
    truncate_lines_at: usize,
    #[serde(default = "default_head_lines")]
    head_lines: usize,
    #[serde(default = "default_tail_lines")]
    tail_lines: usize,
    #[serde(default = "default_max_lines")]
    max_lines: usize,
    #[serde(default)]
    on_empty: String,
    #[serde(default)]
    section: Vec<CommandRuleSectionToml>,
    #[serde(default)]
    counter: Vec<CommandRuleCounterToml>,
    #[serde(default)]
    capture: Vec<CommandRuleCaptureToml>,
    #[serde(default)]
    severity: Vec<CommandRuleSeverityToml>,
    #[serde(default)]
    parse_json: Vec<CommandRuleJsonExtractToml>,
    #[serde(default)]
    parse_ndjson: Vec<CommandRuleJsonExtractToml>,
    #[serde(default)]
    parse_kv: Vec<CommandRuleKvExtractToml>,
    #[serde(default)]
    parse_table: Vec<CommandRuleTableExtractToml>,
    #[serde(default)]
    metric: Vec<CommandRuleMetricToml>,
    #[serde(default)]
    group: Vec<CommandRuleGroupToml>,
    #[serde(default)]
    human_auto_safe: bool,
    #[serde(default = "default_true")]
    agent_safe: bool,
    #[serde(default = "default_interactive_risk")]
    interactive_risk: String,
    #[serde(default, rename = "override")]
    override_config: Option<CommandRuleOverrideToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleOverrideToml {
    #[serde(default)]
    built_in: bool,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleMatchToml {
    #[serde(default)]
    argv_prefix: Vec<String>,
    #[serde(default)]
    command_regex: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleSectionToml {
    name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    preserve_lines_matching: Vec<String>,
    #[serde(default)]
    strip_lines_matching: Vec<String>,
    #[serde(default)]
    keep_lines_matching: Vec<String>,
    #[serde(default = "default_truncate_lines_at")]
    truncate_lines_at: usize,
    #[serde(default = "default_head_lines")]
    head_lines: usize,
    #[serde(default = "default_tail_lines")]
    tail_lines: usize,
    #[serde(default = "default_max_lines")]
    max_lines: usize,
    #[serde(default = "default_section_empty")]
    empty: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleCounterToml {
    name: String,
    #[serde(rename = "match")]
    match_pattern: String,
    #[serde(default = "default_counter_max_count")]
    max_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleCaptureToml {
    name: String,
    pattern: String,
    field: String,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default = "default_capture_max_items")]
    max_items: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleSeverityToml {
    level: String,
    #[serde(rename = "match")]
    match_pattern: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleJsonExtractToml {
    name: String,
    path: String,
    #[serde(default = "default_capture_max_items")]
    max_items: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleKvExtractToml {
    name: String,
    key: String,
    #[serde(default)]
    separators: Vec<String>,
    #[serde(default = "default_capture_max_items")]
    max_items: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleTableExtractToml {
    name: String,
    columns: Vec<String>,
    #[serde(default = "default_table_delimiter")]
    delimiter: String,
    #[serde(default = "default_table_max_rows")]
    max_rows: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleMetricToml {
    name: String,
    op: String,
    #[serde(rename = "match")]
    match_pattern: String,
    #[serde(default = "default_counter_max_count")]
    max_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRuleGroupToml {
    name: String,
    pattern: String,
    field: String,
    #[serde(default = "default_group_top_k")]
    top_k: usize,
}

fn default_true() -> bool {
    true
}

fn default_truncate_lines_at() -> usize {
    180
}

fn default_head_lines() -> usize {
    16
}

fn default_tail_lines() -> usize {
    8
}

fn default_max_lines() -> usize {
    32
}

fn default_interactive_risk() -> String {
    "none".into()
}

fn default_section_empty() -> String {
    "omit".into()
}

fn default_counter_max_count() -> usize {
    10_000
}

fn default_capture_max_items() -> usize {
    25
}

fn default_table_delimiter() -> String {
    "whitespace".into()
}

fn default_table_max_rows() -> usize {
    25
}

fn default_group_top_k() -> usize {
    10
}

impl CommandRuleDiagnostic {
    fn new(source_kind: &str, path: &Path, code: &str, message: impl Into<String>) -> Self {
        Self {
            source_kind: source_kind.into(),
            path: path.display().to_string(),
            code: code.into(),
            message: message.into(),
        }
    }
}

impl CommandRuleSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn diagnostics(&self) -> &[CommandRuleDiagnostic] {
        &self.diagnostics
    }

    pub fn has_builtin_overrides(&self) -> bool {
        self.rules.iter().any(|rule| rule.override_policy.built_in)
    }

    pub fn load_standard(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let mut set = Self::default();
        if let Some(repo_rules) = find_repo_rules(cwd) {
            let tfy_dir = repo_rules.parent().unwrap_or(cwd).to_path_buf();
            let trust = tfy_dir.join("trust.json");
            let repo_root = tfy_dir.parent().unwrap_or(&tfy_dir).to_path_buf();
            let fixture_root = repo_root.join(".tfy").join("rule-fixtures");
            set.load_trusted_custom_source(TrustedSourceConfig {
                source_kind: "repo",
                label: "repo-local",
                rules_path: repo_rules,
                trust_path: trust,
                path_base: repo_root,
                fixture_root,
                untrusted_code: "repo_rules_untrusted",
                hash_code: "repo_rules_hash_mismatch",
                invalid_code: "repo_rules_hash_mismatch",
                symlink_code: "repo_rules_hash_mismatch",
                untrusted_message: "repo-local command rules are not trusted; run tfy custom verify and tfy custom trust after reviewing them",
            });
        }
        if let Some(global_rules) = global_custom_rules_path() {
            if global_rules.exists() {
                let global_root = global_rules
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf();
                let trust = global_root.join("trust.json");
                let fixture_root = global_root.join("rule-fixtures");
                set.load_trusted_custom_source(TrustedSourceConfig {
                    source_kind: "global_custom",
                    label: "global custom",
                    rules_path: global_rules,
                    trust_path: trust,
                    path_base: global_root.clone(),
                    fixture_root,
                    untrusted_code: "global_custom_rules_untrusted",
                    hash_code: "global_custom_rules_hash_mismatch",
                    invalid_code: "global_custom_rules_invalid_trust",
                    symlink_code: "global_custom_rules_symlink_refused",
                    untrusted_message: "global custom command rules are not trusted; run tfy custom verify --scope global and tfy custom trust --scope global after reviewing them",
                });
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user_rules = PathBuf::from(home)
                .join(".config")
                .join("tfy")
                .join("commands.toml");
            if user_rules.exists() {
                set.diagnostics.push(CommandRuleDiagnostic::new(
                    "user",
                    &user_rules,
                    "user_global_rules_legacy_manual",
                    "user-global command rules loaded as legacy/manual compatibility; tfy custom trusted global rules use ~/.config/tfy/custom/",
                ));
                set.load_file_non_strict(&user_rules, "user");
            }
        }
        set
    }

    fn load_trusted_custom_source(&mut self, config: TrustedSourceConfig) {
        match rules_trust_status(&config) {
            Ok(RuleTrustStatus::Trusted) => {
                self.load_file_non_strict(&config.rules_path, config.source_kind);
            }
            Ok(RuleTrustStatus::Untrusted) => self.diagnostics.push(CommandRuleDiagnostic::new(
                config.source_kind,
                &config.rules_path,
                config.untrusted_code,
                config.untrusted_message,
            )),
            Err(message) => {
                let message = message.to_string();
                let code = if message.contains("changed after trust") {
                    config.hash_code
                } else if message.contains("symlink") {
                    config.symlink_code
                } else {
                    config.invalid_code
                };
                self.diagnostics.push(CommandRuleDiagnostic::new(
                    config.source_kind,
                    &config.rules_path,
                    code,
                    message,
                ));
            }
        }
    }

    pub fn load_strict(path: impl AsRef<Path>, source_kind: &str) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("read {}: {err}", path.display()))?;
        let rules = parse_command_rules_strict(&text, source_kind, path)?;
        Ok(Self {
            rules,
            diagnostics: Vec::new(),
        })
    }

    pub fn from_toml_str_strict(text: &str, source_kind: &str) -> Result<Self> {
        Ok(Self {
            rules: parse_command_rules_strict(text, source_kind, Path::new("<memory>"))?,
            diagnostics: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_toml_str_non_strict_for_tests(text: &str, source_kind: &str) -> Self {
        let (rules, diagnostics) =
            parse_command_rules_non_strict(text, source_kind, Path::new("<memory>"));
        Self { rules, diagnostics }
    }

    fn load_file_non_strict(&mut self, path: &Path, source_kind: &str) {
        match fs::read_to_string(path) {
            Ok(text) => {
                let (mut rules, mut diagnostics) =
                    parse_command_rules_non_strict(&text, source_kind, path);
                self.rules.append(&mut rules);
                self.diagnostics.append(&mut diagnostics);
            }
            Err(err) => self.diagnostics.push(CommandRuleDiagnostic::new(
                source_kind,
                path,
                "user_rules_invalid_toml",
                format!("read {}: {err}", path.display()),
            )),
        }
    }

    fn first_match<'a>(
        &'a self,
        argv: &[String],
        command_display: &str,
    ) -> Option<&'a CommandRule> {
        self.rules
            .iter()
            .find(|rule| rule.matches(argv, command_display))
    }

    #[allow(clippy::too_many_arguments)]
    fn summary_candidate(
        &self,
        argv: &[String],
        command_display: &str,
        code: i32,
        raw: &str,
        evidence: &[String],
        rr: &str,
        risk: &str,
    ) -> Option<UserRuleSummaryCandidate> {
        let rule = self.first_match(argv, command_display)?;
        let parts = evaluate_rule(rule, raw)?;
        let context = RuleRenderContext {
            command_display,
            code,
            raw,
            evidence,
            rr,
            risk,
        };
        let text = render_rule_summary(rule, &parts, &context);
        Some(UserRuleSummaryCandidate {
            text,
            rule_id: rule.id.clone(),
            strategy_source_kind: rule.source_kind.clone(),
            human_auto_safe: rule.safety.human_auto_safe,
            agent_safe: rule.safety.agent_safe,
            interactive_risk: rule.safety.interactive_risk.clone(),
            override_policy: rule.override_policy.clone(),
        })
    }
}

struct UserRuleSummaryCandidate {
    text: String,
    rule_id: String,
    strategy_source_kind: String,
    human_auto_safe: bool,
    agent_safe: bool,
    interactive_risk: String,
    override_policy: RuleOverridePolicy,
}

impl CommandRule {
    fn matches(&self, argv: &[String], command_display: &str) -> bool {
        let argv_matches = !self.argv_prefix.is_empty()
            && argv.len() >= self.argv_prefix.len()
            && argv
                .iter()
                .zip(self.argv_prefix.iter())
                .all(|(actual, expected)| actual == expected);
        argv_matches
            || self
                .command_regex
                .as_ref()
                .is_some_and(|re| re.is_match(command_display))
    }
}

fn find_repo_rules(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .map(|dir| dir.join(".tfy").join("commands.toml"))
        .find(|path| path.exists())
}

fn global_custom_rules_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("tfy")
            .join("custom")
            .join("commands.toml")
    })
}

struct TrustedSourceConfig {
    source_kind: &'static str,
    label: &'static str,
    rules_path: PathBuf,
    trust_path: PathBuf,
    path_base: PathBuf,
    fixture_root: PathBuf,
    untrusted_code: &'static str,
    hash_code: &'static str,
    invalid_code: &'static str,
    symlink_code: &'static str,
    untrusted_message: &'static str,
}

enum RuleTrustStatus {
    Trusted,
    Untrusted,
}

#[derive(Deserialize)]
struct RepoTrustFile {
    schema_version: u64,
    command_rules: RepoTrustCommandRules,
}

#[derive(Deserialize)]
struct RepoTrustCommandRules {
    trusted: bool,
    rules_sha256: Option<String>,
    #[serde(default)]
    created_by: Option<String>,
    #[serde(default)]
    validated_at: Option<String>,
    #[serde(default)]
    validated_with: Vec<String>,
    #[serde(default)]
    fixtures: Vec<RepoTrustFixture>,
    #[serde(default)]
    agent: Option<RepoTrustAgent>,
    #[serde(default)]
    override_evidence: Vec<RepoTrustOverrideEvidence>,
}

#[derive(Deserialize)]
struct RepoTrustFixture {
    name: String,
    fixture_sha256: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    cmd: Vec<String>,
}

#[derive(Deserialize)]
struct RepoTrustAgent {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    bounded_workspace: bool,
}

#[derive(Deserialize)]
struct RepoTrustOverrideEvidence {
    #[serde(default)]
    rule_id: String,
    #[serde(default)]
    family: String,
    #[serde(default)]
    override_active: bool,
}

fn rules_trust_status(config: &TrustedSourceConfig) -> Result<RuleTrustStatus> {
    if !config.trust_path.exists() {
        return Ok(RuleTrustStatus::Untrusted);
    }
    reject_symlink_path(&config.rules_path)?;
    if let Some(root) = config.rules_path.parent() {
        reject_symlink_path(root)?;
    }
    reject_symlink_path(&config.trust_path)?;
    let trust_text = fs::read_to_string(&config.trust_path)?;
    let trust: RepoTrustFile = serde_json::from_str(&trust_text)?;
    if !trust.command_rules.trusted {
        return Ok(RuleTrustStatus::Untrusted);
    }
    match trust.schema_version {
        1 => Ok(RuleTrustStatus::Untrusted),
        2 => {
            let Some(expected) = trust.command_rules.rules_sha256.as_deref() else {
                return Ok(RuleTrustStatus::Untrusted);
            };
            let actual = sha256_hex(&fs::read(&config.rules_path)?);
            if expected != actual {
                bail!(
                    "{} command rules changed after trust; expected sha256 {expected}, got {actual}",
                    config.label
                );
            }
            validate_trust_v2(config, &trust.command_rules)?;
            Ok(RuleTrustStatus::Trusted)
        }
        _ => Ok(RuleTrustStatus::Untrusted),
    }
}

fn validate_trust_v2(
    config: &TrustedSourceConfig,
    command_rules: &RepoTrustCommandRules,
) -> Result<()> {
    if command_rules.created_by.as_deref() != Some("tfy custom") {
        bail!(
            "{} command rules v2 trust must be created_by=tfy custom",
            config.label
        );
    }
    let validated_at = command_rules.validated_at.as_deref().unwrap_or("");
    if validated_at.is_empty() {
        bail!(
            "{} command rules v2 trust is missing validated_at",
            config.label
        );
    }
    for required in ["validate", "preview", "compare-built-in"] {
        if !command_rules
            .validated_with
            .iter()
            .any(|item| item == required)
        {
            bail!(
                "{} command rules v2 trust is missing validation evidence {required}",
                config.label
            );
        }
    }
    let Some(agent) = &command_rules.agent else {
        bail!(
            "{} command rules v2 trust is missing agent provenance",
            config.label
        );
    };
    if agent.kind.is_empty() {
        bail!(
            "{} command rules v2 trust is missing agent kind",
            config.label
        );
    }
    if !agent.bounded_workspace {
        bail!(
            "{} command rules v2 trust requires bounded_workspace=true",
            config.label
        );
    }
    if command_rules.fixtures.is_empty() {
        bail!(
            "{} command rules v2 trust requires at least one fixture",
            config.label
        );
    }
    reject_symlink_path(&config.fixture_root)?;
    let fixture_root = config.fixture_root.canonicalize()?;
    for fixture in &command_rules.fixtures {
        if fixture.name.is_empty() || fixture.fixture_sha256.is_empty() || fixture.cmd.is_empty() {
            bail!(
                "{} command rules v2 trust has incomplete fixture metadata",
                config.label
            );
        }
        let relative = fixture
            .path
            .clone()
            .unwrap_or_else(|| format!("rule-fixtures/{}.txt", fixture.name));
        let raw_fixture_path = PathBuf::from(relative);
        let fixture_path = if raw_fixture_path.is_absolute() {
            raw_fixture_path
        } else {
            config.path_base.join(raw_fixture_path)
        };
        let metadata = fs::symlink_metadata(&fixture_path)?;
        if metadata.file_type().is_symlink() {
            bail!(
                "{} command rules v2 fixture path is a symlink",
                config.label
            );
        }
        let fixture_path = fixture_path.canonicalize()?;
        if !fixture_path.starts_with(&fixture_root) {
            bail!(
                "{} command rules v2 fixture path escapes fixture root",
                config.label
            );
        }
        let actual = sha256_hex(&fs::read(&fixture_path)?);
        if actual != fixture.fixture_sha256 {
            bail!(
                "{} command rules fixture {} changed after trust; expected sha256 {}, got {actual}",
                config.label,
                fixture.name,
                fixture.fixture_sha256
            );
        }
    }
    let rules = CommandRuleSet::load_strict(&config.rules_path, config.source_kind)?;
    if rules.has_builtin_overrides() {
        let has_active = command_rules
            .override_evidence
            .iter()
            .any(|e| e.override_active && !e.rule_id.is_empty() && !e.family.is_empty());
        if !has_active {
            bail!(
                "{} command rules v2 trust is missing active override evidence",
                config.label
            );
        }
    }
    let metadata = fs::symlink_metadata(&config.trust_path)?;
    if metadata.file_type().is_symlink() {
        bail!("{} command rules v2 trust path is a symlink", config.label);
    }
    Ok(())
}

fn reject_symlink_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!(
            "command rules trust refuses symlink path: {}",
            path.display()
        );
    }
    Ok(())
}

fn parse_command_rules_strict(
    text: &str,
    source_kind: &str,
    path: &Path,
) -> Result<Vec<CommandRule>> {
    let parsed: CommandRulesFile =
        toml::from_str(text).map_err(|err| anyhow::anyhow!("invalid TOML: {err}"))?;
    let schema_version = validate_schema_version(parsed.schema_version)?;
    let mut ids = std::collections::BTreeSet::new();
    parsed
        .command
        .into_iter()
        .map(|rule| build_command_rule(rule, source_kind, schema_version, &mut ids))
        .collect::<Result<Vec<_>>>()
        .map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))
}

fn parse_command_rules_non_strict(
    text: &str,
    source_kind: &str,
    path: &Path,
) -> (Vec<CommandRule>, Vec<CommandRuleDiagnostic>) {
    let parsed = match text.parse::<toml::Value>() {
        Ok(parsed) => parsed,
        Err(err) => {
            return (
                Vec::new(),
                vec![CommandRuleDiagnostic::new(
                    source_kind,
                    path,
                    "user_rules_invalid_toml",
                    format!("invalid TOML: {err}"),
                )],
            );
        }
    };

    let mut diagnostics = Vec::new();
    if let Some(table) = parsed.as_table() {
        for key in table
            .keys()
            .filter(|key| !matches!(key.as_str(), "command" | "schema_version"))
        {
            diagnostics.push(CommandRuleDiagnostic::new(
                source_kind,
                path,
                "user_rules_unsupported_field",
                format!("unknown top-level field {key:?}"),
            ));
        }
    }
    let schema_version = match parsed.get("schema_version") {
        Some(value) => match value
            .as_integer()
            .and_then(|version| u16::try_from(version).ok())
        {
            Some(version) => match validate_schema_version(Some(version)) {
                Ok(version) => version,
                Err(err) => {
                    diagnostics.push(CommandRuleDiagnostic::new(
                        source_kind,
                        path,
                        "user_rules_unsupported_schema_version",
                        err.to_string(),
                    ));
                    return (Vec::new(), diagnostics);
                }
            },
            None => {
                diagnostics.push(CommandRuleDiagnostic::new(
                    source_kind,
                    path,
                    "user_rules_unsupported_schema_version",
                    "schema_version must be integer 1, 2, or 3",
                ));
                return (Vec::new(), diagnostics);
            }
        },
        None => 1,
    };
    let Some(commands) = parsed.get("command") else {
        return (Vec::new(), diagnostics);
    };
    let Some(commands) = commands.as_array() else {
        return (
            Vec::new(),
            vec![CommandRuleDiagnostic::new(
                source_kind,
                path,
                "user_rules_invalid_toml",
                "command rules must use [[command]] arrays",
            )],
        );
    };

    let mut ids = std::collections::BTreeSet::new();
    let mut rules = Vec::new();
    for (index, value) in commands.iter().enumerate() {
        let raw_rule = match value.clone().try_into::<CommandRuleToml>() {
            Ok(rule) => rule,
            Err(err) => {
                diagnostics.push(CommandRuleDiagnostic::new(
                    source_kind,
                    path,
                    diagnostic_code_from_message(&err.to_string()),
                    format!("command[{index}]: {err}"),
                ));
                continue;
            }
        };
        match build_command_rule(raw_rule, source_kind, schema_version, &mut ids) {
            Ok(rule) => rules.push(rule),
            Err(err) => diagnostics.push(CommandRuleDiagnostic::new(
                source_kind,
                path,
                diagnostic_code_from_message(&err.to_string()),
                format!("command[{index}]: {err}"),
            )),
        }
    }
    (rules, diagnostics)
}

fn build_command_rule(
    rule: CommandRuleToml,
    source_kind: &str,
    schema_version: u16,
    ids: &mut std::collections::BTreeSet<String>,
) -> Result<CommandRule> {
    if rule.id.trim().is_empty() {
        bail!("command rule id must not be empty");
    }
    validate_safe_name(&rule.id, "command rule id")?;
    if !ids.insert(rule.id.clone()) {
        bail!("duplicate command rule id: {}", rule.id);
    }
    if rule.match_config.argv_prefix.is_empty()
        && rule
            .match_config
            .command_regex
            .as_deref()
            .unwrap_or("")
            .is_empty()
    {
        bail!(
            "command rule {} needs match.argv_prefix or match.command_regex",
            rule.id
        );
    }
    validate_limit(rule.truncate_lines_at, 10_000, "truncate_lines_at")?;
    validate_limit(rule.head_lines, 1_000, "head_lines")?;
    validate_limit(rule.tail_lines, 1_000, "tail_lines")?;
    validate_limit(rule.max_lines, 1_000, "max_lines")?;
    validate_limit(rule.on_empty.chars().count(), 512, "on_empty")?;
    validate_pattern_list(&rule.preserve_lines_matching, "preserve_lines_matching")?;
    validate_pattern_list(&rule.strip_lines_matching, "strip_lines_matching")?;
    validate_pattern_list(&rule.keep_lines_matching, "keep_lines_matching")?;
    if schema_version < 2
        && (!rule.section.is_empty()
            || !rule.counter.is_empty()
            || !rule.capture.is_empty()
            || !rule.severity.is_empty())
    {
        bail!(
            "command rule {} uses v2 operations without schema_version >= 2",
            rule.id
        );
    }
    if schema_version < 3
        && (!rule.parse_json.is_empty()
            || !rule.parse_ndjson.is_empty()
            || !rule.parse_kv.is_empty()
            || !rule.parse_table.is_empty()
            || !rule.metric.is_empty()
            || !rule.group.is_empty())
    {
        bail!(
            "command rule {} uses v3 operations without schema_version = 3",
            rule.id
        );
    }
    if !matches!(
        rule.interactive_risk.as_str(),
        "none" | "possible" | "unknown"
    ) {
        bail!(
            "command rule {} has invalid interactive_risk {}",
            rule.id,
            rule.interactive_risk
        );
    }
    let override_policy = build_override_policy(&rule, schema_version)?;
    let command_regex = rule
        .match_config
        .command_regex
        .as_deref()
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| compile_user_regex(pattern, "command_regex"))
        .transpose()?;
    let mut operations = Vec::new();
    operations.push(RuleOperation::LineFilter(RuntimeFilter {
        strip_ansi: rule.strip_ansi,
        strip_lines_matching: compile_user_regexes(&rule.strip_lines_matching)?,
        keep_lines_matching: compile_user_regexes(&rule.keep_lines_matching)?,
        preserve_lines_matching: compile_user_regexes(&rule.preserve_lines_matching)?,
        truncate_lines_at: rule.truncate_lines_at,
        head_lines: rule.head_lines,
        tail_lines: rule.tail_lines,
        max_lines: rule.max_lines,
        on_empty: if rule.on_empty.is_empty() {
            format!("{}: no relevant output", rule.id)
        } else {
            cap_to_chars(&norm(&redact_public(&rule.on_empty)), 512)
        },
    }));
    for section in rule.section {
        operations.push(RuleOperation::Section(build_section_spec(section)?));
    }
    for counter in rule.counter {
        operations.push(RuleOperation::Counter(build_counter_spec(counter)?));
    }
    for capture in rule.capture {
        operations.push(RuleOperation::Capture(build_capture_spec(capture)?));
    }
    for severity in rule.severity {
        operations.push(RuleOperation::Severity(build_severity_spec(severity)?));
    }
    for extract in rule.parse_json {
        operations.push(RuleOperation::StructuredExtract(
            StructuredExtractSpec::Json(build_json_extract_spec(extract, "parse_json")?),
        ));
    }
    for extract in rule.parse_ndjson {
        operations.push(RuleOperation::StructuredExtract(
            StructuredExtractSpec::Ndjson(build_json_extract_spec(extract, "parse_ndjson")?),
        ));
    }
    for extract in rule.parse_kv {
        operations.push(RuleOperation::StructuredExtract(StructuredExtractSpec::Kv(
            build_kv_extract_spec(extract)?,
        )));
    }
    for extract in rule.parse_table {
        operations.push(RuleOperation::StructuredExtract(
            StructuredExtractSpec::Table(build_table_extract_spec(extract)?),
        ));
    }
    for metric in rule.metric {
        operations.push(RuleOperation::Metric(build_metric_spec(metric)?));
    }
    for group in rule.group {
        operations.push(RuleOperation::Group(build_group_spec(group)?));
    }
    Ok(CommandRule {
        id: rule.id,
        source_kind: source_kind.into(),
        argv_prefix: rule.match_config.argv_prefix,
        command_regex,
        safety: RuleSafetyMetadata {
            human_auto_safe: rule.human_auto_safe,
            agent_safe: rule.agent_safe,
            interactive_risk: rule.interactive_risk,
        },
        override_policy,
        operations,
    })
}

fn build_override_policy(
    rule: &CommandRuleToml,
    schema_version: u16,
) -> Result<RuleOverridePolicy> {
    let Some(override_config) = &rule.override_config else {
        return Ok(RuleOverridePolicy::default());
    };
    if schema_version < 3 {
        bail!(
            "command rule {} uses override without schema_version = 3",
            rule.id
        );
    }
    if !override_config.built_in {
        return Ok(RuleOverridePolicy::default());
    }
    let Some(family) = override_config.family.as_deref() else {
        bail!(
            "command rule {} override.built_in requires override.family",
            rule.id
        );
    };
    validate_safe_name(family, "override.family")?;
    let reason = match override_config.reason.as_deref() {
        Some(reason) => {
            validate_limit(reason.chars().count(), 240, "override.reason")?;
            let reason = cap_to_chars(&norm(&redact_public(reason)), 240);
            if reason.is_empty() {
                None
            } else {
                Some(reason)
            }
        }
        None => None,
    };
    Ok(RuleOverridePolicy {
        built_in: true,
        family: Some(family.to_string()),
        reason,
    })
}

fn build_section_spec(section: CommandRuleSectionToml) -> Result<SectionSpec> {
    validate_safe_name(&section.name, "section name")?;
    validate_limit(section.title.chars().count(), 120, "section title")?;
    validate_limit(
        section.truncate_lines_at,
        10_000,
        "section.truncate_lines_at",
    )?;
    validate_limit(section.head_lines, 1_000, "section.head_lines")?;
    validate_limit(section.tail_lines, 1_000, "section.tail_lines")?;
    validate_limit(section.max_lines, 1_000, "section.max_lines")?;
    validate_pattern_list(
        &section.preserve_lines_matching,
        "section.preserve_lines_matching",
    )?;
    validate_pattern_list(
        &section.strip_lines_matching,
        "section.strip_lines_matching",
    )?;
    validate_pattern_list(&section.keep_lines_matching, "section.keep_lines_matching")?;
    let empty = match section.empty.as_str() {
        "omit" | "none" => SectionEmpty::Omit,
        "show" => SectionEmpty::Show,
        other => bail!("invalid section empty policy {other}"),
    };
    Ok(SectionSpec {
        title: if section.title.is_empty() {
            section.name.clone()
        } else {
            cap_to_chars(&norm(&redact_public(&section.title)), 120)
        },
        filter: RuntimeFilter {
            strip_ansi: true,
            strip_lines_matching: compile_user_regexes(&section.strip_lines_matching)?,
            keep_lines_matching: compile_user_regexes(&section.keep_lines_matching)?,
            preserve_lines_matching: compile_user_regexes(&section.preserve_lines_matching)?,
            truncate_lines_at: section.truncate_lines_at,
            head_lines: section.head_lines,
            tail_lines: section.tail_lines,
            max_lines: section.max_lines,
            on_empty: String::new(),
        },
        empty,
    })
}

fn build_counter_spec(counter: CommandRuleCounterToml) -> Result<CounterSpec> {
    validate_safe_name(&counter.name, "counter name")?;
    validate_limit(counter.max_count, 1_000_000, "counter.max_count")?;
    Ok(CounterSpec {
        name: counter.name,
        pattern: compile_user_regex(&counter.match_pattern, "counter.match")?,
        max_count: counter.max_count,
    })
}

fn build_capture_spec(capture: CommandRuleCaptureToml) -> Result<CaptureSpec> {
    validate_safe_name(&capture.name, "capture name")?;
    validate_safe_name(&capture.field, "capture field")?;
    validate_limit(capture.max_items, 1_000, "capture.max_items")?;
    let pattern = compile_user_regex(&capture.pattern, "capture.pattern")?;
    if pattern
        .capture_names()
        .flatten()
        .all(|name| name != capture.field)
    {
        bail!(
            "capture {} missing named field {}",
            capture.name,
            capture.field
        );
    }
    Ok(CaptureSpec {
        name: capture.name,
        pattern,
        field: capture.field,
        dedupe: capture.dedupe,
        max_items: capture.max_items,
    })
}

fn build_severity_spec(severity: CommandRuleSeverityToml) -> Result<SeveritySpec> {
    let level = match severity.level.as_str() {
        "info" => SeverityLevel::Info,
        "warning" => SeverityLevel::Warning,
        "error" => SeverityLevel::Error,
        "critical" => SeverityLevel::Critical,
        other => bail!("invalid severity level {other}"),
    };
    Ok(SeveritySpec {
        level,
        pattern: compile_user_regex(&severity.match_pattern, "severity.match")?,
    })
}

fn build_json_extract_spec(
    extract: CommandRuleJsonExtractToml,
    label: &str,
) -> Result<JsonExtractSpec> {
    validate_safe_name(&extract.name, "extract name")?;
    validate_limit(extract.path.chars().count(), 240, label)?;
    validate_limit(
        extract.max_items,
        USER_RULE_STRUCTURED_MAX_ITEMS,
        "extract.max_items",
    )?;
    Ok(JsonExtractSpec {
        name: extract.name,
        path: extract.path,
        max_items: extract.max_items,
    })
}

fn build_kv_extract_spec(extract: CommandRuleKvExtractToml) -> Result<KvExtractSpec> {
    validate_safe_name(&extract.name, "extract name")?;
    validate_limit(extract.key.chars().count(), 120, "parse_kv.key")?;
    validate_limit(
        extract.max_items,
        USER_RULE_STRUCTURED_MAX_ITEMS,
        "parse_kv.max_items",
    )?;
    let separators = if extract.separators.is_empty() {
        vec!['=', ':']
    } else {
        let mut separators = Vec::new();
        for separator in extract.separators {
            let mut chars = separator.chars();
            let Some(ch) = chars.next() else {
                bail!("parse_kv separator must not be empty");
            };
            if chars.next().is_some() {
                bail!("parse_kv separator must be one character");
            }
            separators.push(ch);
        }
        separators
    };
    Ok(KvExtractSpec {
        name: extract.name,
        key: extract.key,
        separators,
        max_items: extract.max_items,
    })
}

fn build_table_extract_spec(extract: CommandRuleTableExtractToml) -> Result<TableExtractSpec> {
    validate_safe_name(&extract.name, "extract name")?;
    validate_limit(extract.columns.len(), 20, "parse_table.columns")?;
    validate_limit(
        extract.max_rows,
        USER_RULE_STRUCTURED_MAX_ITEMS,
        "parse_table.max_rows",
    )?;
    if extract.columns.is_empty() {
        bail!("parse_table.columns must not be empty");
    }
    for column in &extract.columns {
        validate_safe_name(column, "table column")?;
    }
    let delimiter = match extract.delimiter.as_str() {
        "whitespace" => TableDelimiter::Whitespace,
        "comma" | "csv" => TableDelimiter::Comma,
        other => bail!("invalid parse_table delimiter {other}"),
    };
    Ok(TableExtractSpec {
        name: extract.name,
        columns: extract.columns,
        delimiter,
        max_rows: extract.max_rows,
    })
}

fn build_metric_spec(metric: CommandRuleMetricToml) -> Result<MetricSpec> {
    validate_safe_name(&metric.name, "metric name")?;
    validate_limit(metric.max_count, 1_000_000, "metric.max_count")?;
    let op = match metric.op.as_str() {
        "count" => MetricOp::Count,
        "unique_count" => MetricOp::UniqueCount,
        other => bail!("invalid metric op {other}"),
    };
    Ok(MetricSpec {
        name: metric.name,
        op,
        pattern: compile_user_regex(&metric.match_pattern, "metric.match")?,
        max_count: metric.max_count,
    })
}

fn build_group_spec(group: CommandRuleGroupToml) -> Result<GroupSpec> {
    validate_safe_name(&group.name, "group name")?;
    validate_safe_name(&group.field, "group field")?;
    validate_limit(group.top_k, 100, "group.top_k")?;
    let pattern = compile_user_regex(&group.pattern, "group.pattern")?;
    if pattern
        .capture_names()
        .flatten()
        .all(|name| name != group.field)
    {
        bail!("group {} missing named field {}", group.name, group.field);
    }
    Ok(GroupSpec {
        name: group.name,
        pattern,
        field: group.field,
        top_k: group.top_k,
    })
}

#[derive(Default)]
struct SummaryParts {
    line_filter: Option<FilteredOutput>,
    sections: Vec<RenderedSection>,
    counters: Vec<(String, usize)>,
    captures: Vec<RenderedCapture>,
    max_severity: Option<SeverityLevel>,
}

struct RenderedSection {
    title: String,
    lines: Vec<String>,
    selected_lines: usize,
}

struct RenderedCapture {
    name: String,
    items: Vec<String>,
}

fn evaluate_rule(rule: &CommandRule, raw: &str) -> Option<SummaryParts> {
    let mut parts = SummaryParts::default();
    let public_lines = public_rule_lines(raw);
    for operation in &rule.operations {
        match operation {
            RuleOperation::LineFilter(filter) => {
                parts.line_filter = apply_runtime_filter(filter, raw);
            }
            RuleOperation::Section(section) => {
                if let Some(filtered) = apply_runtime_filter(&section.filter, raw) {
                    if !filtered.lines.is_empty() || section.empty == SectionEmpty::Show {
                        parts.sections.push(RenderedSection {
                            title: section.title.clone(),
                            lines: filtered.lines,
                            selected_lines: filtered.selected_lines,
                        });
                    }
                }
            }
            RuleOperation::Counter(counter) => {
                let mut count = 0usize;
                for line in &public_lines {
                    count = count.saturating_add(counter.pattern.find_iter(line).count());
                    if count >= counter.max_count {
                        count = counter.max_count;
                        break;
                    }
                }
                parts.counters.push((counter.name.clone(), count));
            }
            RuleOperation::Capture(capture) => {
                let mut items = Vec::new();
                let mut seen = std::collections::BTreeSet::new();
                for line in &public_lines {
                    for caps in capture.pattern.captures_iter(line) {
                        let Some(value) = caps.name(&capture.field) else {
                            continue;
                        };
                        let item = cap_to_chars(&norm(&redact_public(value.as_str())), 240);
                        if item.is_empty() {
                            continue;
                        }
                        if capture.dedupe && !seen.insert(item.clone()) {
                            continue;
                        }
                        items.push(item);
                        if items.len() >= capture.max_items {
                            break;
                        }
                    }
                    if items.len() >= capture.max_items {
                        break;
                    }
                }
                if !items.is_empty() {
                    parts.captures.push(RenderedCapture {
                        name: capture.name.clone(),
                        items,
                    });
                }
            }
            RuleOperation::Severity(severity) => {
                if public_lines
                    .iter()
                    .any(|line| severity.pattern.is_match(line))
                {
                    parts.max_severity =
                        Some(parts.max_severity.map_or(severity.level, |current| {
                            std::cmp::max(current, severity.level)
                        }));
                }
            }
            RuleOperation::StructuredExtract(extract) => {
                if let Some(capture) = evaluate_structured_extract(extract, raw, &public_lines) {
                    parts.captures.push(capture);
                }
            }
            RuleOperation::Metric(metric) => {
                let value = evaluate_metric(metric, &public_lines);
                parts.counters.push((metric.name.clone(), value));
            }
            RuleOperation::Group(group) => {
                if let Some(section) = evaluate_group(group, &public_lines) {
                    parts.sections.push(section);
                }
            }
        }
    }
    if parts.line_filter.is_none()
        && parts.sections.is_empty()
        && parts.counters.is_empty()
        && parts.captures.is_empty()
        && parts.max_severity.is_none()
    {
        None
    } else {
        Some(parts)
    }
}

fn evaluate_structured_extract(
    extract: &StructuredExtractSpec,
    raw: &str,
    public_lines: &[String],
) -> Option<RenderedCapture> {
    match extract {
        StructuredExtractSpec::Json(spec) => evaluate_json_extract(spec, raw),
        StructuredExtractSpec::Ndjson(spec) => evaluate_ndjson_extract(spec, raw),
        StructuredExtractSpec::Kv(spec) => evaluate_kv_extract(spec, public_lines),
        StructuredExtractSpec::Table(spec) => evaluate_table_extract(spec, public_lines),
    }
}

fn evaluate_json_extract(spec: &JsonExtractSpec, raw: &str) -> Option<RenderedCapture> {
    if raw.len() > USER_RULE_STRUCTURED_PARSE_MAX_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let items = json_path_values(&value, &spec.path)
        .into_iter()
        .take(spec.max_items)
        .filter_map(public_json_value)
        .collect::<Vec<_>>();
    rendered_capture(&spec.name, items)
}

fn evaluate_ndjson_extract(spec: &JsonExtractSpec, raw: &str) -> Option<RenderedCapture> {
    if raw.len() > USER_RULE_STRUCTURED_PARSE_MAX_BYTES {
        return None;
    }
    let mut items = Vec::new();
    for line in raw.lines().take(USER_RULE_STRUCTURED_MAX_ITEMS * 10) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        for value in json_path_values(&value, &spec.path) {
            if let Some(item) = public_json_value(value) {
                items.push(item);
            }
            if items.len() >= spec.max_items {
                break;
            }
        }
        if items.len() >= spec.max_items {
            break;
        }
    }
    rendered_capture(&spec.name, items)
}

fn evaluate_kv_extract(spec: &KvExtractSpec, public_lines: &[String]) -> Option<RenderedCapture> {
    let mut items = Vec::new();
    for line in public_lines {
        let trimmed = line.trim();
        for separator in &spec.separators {
            let Some((key, value)) = trimmed.split_once(*separator) else {
                continue;
            };
            if key.trim() == spec.key {
                let item = cap_to_chars(&norm(&redact_public(value.trim())), 240);
                if !item.is_empty() {
                    items.push(item);
                }
            }
            if items.len() >= spec.max_items {
                break;
            }
        }
        if items.len() >= spec.max_items {
            break;
        }
    }
    rendered_capture(&spec.name, items)
}

fn evaluate_table_extract(
    spec: &TableExtractSpec,
    public_lines: &[String],
) -> Option<RenderedCapture> {
    let non_empty = public_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let (header_index, indexes) = non_empty
        .iter()
        .enumerate()
        .find_map(|(line_index, line)| {
            let header = split_table_row(line, spec.delimiter);
            let indexes = spec
                .columns
                .iter()
                .filter_map(|column| {
                    header
                        .iter()
                        .position(|cell| cell == column)
                        .map(|index| (column, index))
                })
                .collect::<Vec<_>>();
            (indexes.len() == spec.columns.len()).then_some((line_index, indexes))
        })?;
    let mut items = Vec::new();
    for line in non_empty
        .into_iter()
        .skip(header_index + 1)
        .take(spec.max_rows)
    {
        let cells = split_table_row(line, spec.delimiter);
        let fields = indexes
            .iter()
            .filter_map(|(column, index)| {
                cells.get(*index).map(|value| {
                    format!(
                        "{column}={}",
                        cap_to_chars(&norm(&redact_public(value)), 120)
                    )
                })
            })
            .collect::<Vec<_>>();
        if !fields.is_empty() {
            items.push(fields.join(" "));
        }
    }
    rendered_capture(&spec.name, items)
}

fn evaluate_metric(spec: &MetricSpec, public_lines: &[String]) -> usize {
    match spec.op {
        MetricOp::Count => {
            let mut count = 0usize;
            for line in public_lines {
                count = count.saturating_add(spec.pattern.find_iter(line).count());
                if count >= spec.max_count {
                    return spec.max_count;
                }
            }
            count
        }
        MetricOp::UniqueCount => {
            let mut seen = std::collections::BTreeSet::new();
            for line in public_lines {
                for m in spec.pattern.find_iter(line) {
                    seen.insert(cap_to_chars(&norm(&redact_public(m.as_str())), 240));
                    if seen.len() >= spec.max_count {
                        return spec.max_count;
                    }
                }
            }
            seen.len()
        }
    }
}

fn evaluate_group(spec: &GroupSpec, public_lines: &[String]) -> Option<RenderedSection> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut truncated = false;
    for line in public_lines {
        for caps in spec.pattern.captures_iter(line) {
            let Some(value) = caps.name(&spec.field) else {
                continue;
            };
            let key = cap_to_chars(&norm(&redact_public(value.as_str())), 240);
            if !key.is_empty() {
                if !counts.contains_key(&key) && counts.len() >= USER_RULE_GROUP_MAX_DISTINCT {
                    truncated = true;
                    continue;
                }
                *counts.entry(key).or_insert(0) += 1;
            }
        }
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut lines = items
        .into_iter()
        .take(spec.top_k)
        .map(|(key, count)| format!("{key}={count}"))
        .collect::<Vec<_>>();
    if truncated {
        lines.push(format!(
            "[truncated distinct groups at {}]",
            USER_RULE_GROUP_MAX_DISTINCT
        ));
    }
    if lines.is_empty() {
        None
    } else {
        Some(RenderedSection {
            title: format!("group.{}", spec.name),
            selected_lines: lines.len(),
            lines,
        })
    }
}

fn rendered_capture(name: &str, items: Vec<String>) -> Option<RenderedCapture> {
    let items = items
        .into_iter()
        .map(|item| cap_to_chars(&norm(&redact_public(&item)), 240))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if items.is_empty() {
        None
    } else {
        Some(RenderedCapture {
            name: name.to_string(),
            items,
        })
    }
}

fn public_json_value(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).ok()?
        }
    };
    let text = cap_to_chars(&norm(&redact_public(&text)), 240);
    (!text.is_empty()).then_some(text)
}

fn json_path_values<'a>(value: &'a serde_json::Value, path: &str) -> Vec<&'a serde_json::Value> {
    let mut current = vec![value];
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        let mut next = Vec::new();
        for value in current {
            next.extend(json_segment_values(value, segment));
        }
        current = next;
        if current.is_empty() {
            break;
        }
    }
    current
}

fn json_segment_values<'a>(
    value: &'a serde_json::Value,
    segment: &str,
) -> Vec<&'a serde_json::Value> {
    let (name, selectors) = segment
        .split_once('[')
        .map_or((segment, ""), |(name, rest)| (name, rest));
    let mut current = Vec::new();
    if name.is_empty() {
        current.push(value);
    } else if let Some(child) = value.get(name) {
        current.push(child);
    }
    let mut rest = selectors;
    while !rest.is_empty() {
        let Some((selector, remaining)) = rest.split_once(']') else {
            return Vec::new();
        };
        let mut selected = Vec::new();
        for value in current {
            match selector {
                "*" => {
                    if let Some(array) = value.as_array() {
                        selected.extend(array);
                    }
                }
                index => {
                    if let Ok(index) = index.parse::<usize>() {
                        if let Some(child) = value.get(index) {
                            selected.push(child);
                        }
                    }
                }
            }
        }
        current = selected;
        rest = remaining.strip_prefix('[').unwrap_or("");
    }
    current
}

fn split_table_row(line: &str, delimiter: TableDelimiter) -> Vec<String> {
    match delimiter {
        TableDelimiter::Whitespace => line.split_whitespace().map(str::to_string).collect(),
        TableDelimiter::Comma => line
            .split(',')
            .map(|cell| cell.trim().to_string())
            .collect(),
    }
}

struct RuleRenderContext<'a> {
    command_display: &'a str,
    code: i32,
    raw: &'a str,
    evidence: &'a [String],
    rr: &'a str,
    risk: &'a str,
}

fn render_rule_summary(
    rule: &CommandRule,
    parts: &SummaryParts,
    context: &RuleRenderContext<'_>,
) -> String {
    let display_risk = match parts.max_severity {
        Some(SeverityLevel::Critical) => "CRITICAL".to_string(),
        Some(SeverityLevel::Error) if context.risk == "success" => "ERROR".to_string(),
        Some(SeverityLevel::Warning) if context.risk == "success" => "WARNING".to_string(),
        _ => context.risk.to_uppercase(),
    };
    let mut lines = vec![format!(
        "TFY command summary: {display_risk} strategy=user_toml rule_id={} source={} exit={} cmd={}",
        rule.id, rule.source_kind, context.code, context.command_display
    )];
    lines.push(format!("- original_lines={}", context.raw.lines().count()));
    if let Some(level) = parts.max_severity {
        lines.push(format!("- custom_severity={}", level.as_str()));
    }
    for (name, count) in &parts.counters {
        lines.push(format!("- counter.{name}={count}"));
    }
    if let Some(filtered) = &parts.line_filter {
        lines.push(format!("- selected_lines={}", filtered.selected_lines));
        for line in filtered.preserved.iter().take(8) {
            lines.push(format!("- preserved: {line}"));
        }
        if !filtered.lines.is_empty() {
            lines.push("Output:".into());
            for line in &filtered.lines {
                lines.push(format!("- {line}"));
            }
        }
    }
    for section in &parts.sections {
        lines.push(format!("{}:", section.title));
        lines.push(format!("- selected_lines={}", section.selected_lines));
        for line in &section.lines {
            lines.push(format!("- {line}"));
        }
    }
    for capture in &parts.captures {
        lines.push(format!("{}:", capture.name));
        for item in &capture.items {
            lines.push(format!("- {item}"));
        }
    }
    if context.risk != "success" {
        lines.extend(
            context
                .evidence
                .iter()
                .take(5)
                .map(|e| format!("- evidence: {e}")),
        );
    }
    lines.push(format!("raw_ref={}", context.rr));
    lines.push(String::new());
    lines
        .into_iter()
        .map(|line| cap_to_chars(&norm(&redact_public(&line)), 1_000))
        .collect::<Vec<_>>()
        .join("\n")
}

fn public_rule_lines(raw: &str) -> Vec<String> {
    strip_ansi_sequences(raw)
        .lines()
        .map(|line| {
            cap_to_chars(
                &norm(&redact_public(line)),
                USER_RULE_PUBLIC_SCAN_LINE_CHARS,
            )
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn validate_schema_version(version: Option<u16>) -> Result<u16> {
    match version.unwrap_or(1) {
        version @ 1..=3 => Ok(version),
        other => bail!("unsupported command rules schema_version {other}"),
    }
}

fn validate_safe_name(value: &str, label: &str) -> Result<()> {
    let len = value.chars().count();
    if len == 0 || len > 64 {
        bail!("{label} must be 1..64 characters");
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        bail!("{label} must contain only ASCII letters, digits, '_' or '-'");
    }
    Ok(())
}

fn validate_limit(value: usize, max: usize, name: &str) -> Result<()> {
    if value > max {
        bail!("{name} exceeds max {max}");
    }
    Ok(())
}

fn validate_pattern_list(patterns: &[String], name: &str) -> Result<()> {
    if patterns.len() > 64 {
        bail!("{name} has too many patterns");
    }
    for pattern in patterns {
        if pattern.len() > 512 {
            bail!("{name} pattern exceeds 512 bytes");
        }
    }
    Ok(())
}

fn compile_user_regexes(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| compile_user_regex(pattern, "pattern"))
        .collect()
}

fn compile_user_regex(pattern: &str, label: &str) -> Result<Regex> {
    if pattern.len() > 512 {
        bail!("{label} exceeds 512 bytes");
    }
    Regex::new(pattern).map_err(|err| anyhow::anyhow!("invalid regex {pattern:?}: {err}"))
}

fn diagnostic_code_from_message(message: &str) -> &'static str {
    if message.contains("unknown field") || message.contains("unknown top-level field") {
        "user_rules_unsupported_field"
    } else if message.contains("invalid TOML") {
        "user_rules_invalid_toml"
    } else if message.contains("regex") {
        "user_rules_invalid_regex"
    } else if message.contains("duplicate") {
        "user_rules_duplicate_id"
    } else if message.contains("unsupported command rules schema_version")
        || message.contains("schema_version")
    {
        "user_rules_unsupported_schema_version"
    } else if message.contains("section") {
        "user_rules_invalid_section"
    } else if message.contains("counter") {
        "user_rules_invalid_counter"
    } else if message.contains("capture") {
        "user_rules_invalid_capture"
    } else if message.contains("severity") {
        "user_rules_invalid_severity"
    } else if message.contains("override") {
        "user_rules_invalid_override"
    } else if message.contains("parse_") || message.contains("extract") {
        "user_rules_invalid_extract"
    } else if message.contains("metric") {
        "user_rules_invalid_metric"
    } else if message.contains("group") {
        "user_rules_invalid_group"
    } else if message.contains("exceeds") || message.contains("too many patterns") {
        "user_rules_unsafe_limit"
    } else {
        "user_rules_unsupported_field"
    }
}

struct StoredRaw {
    raw_ref: String,
    output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct RawStore {
    root: PathBuf,
}

impl RawStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        Ok(Self { root })
    }
    pub fn put(&self, command: &str, raw: &str, exit_code: i32) -> Result<String> {
        self.put_bytes(command, raw.as_bytes(), exit_code)
    }
    pub fn put_bytes(&self, command: &str, raw: &[u8], exit_code: i32) -> Result<String> {
        for attempt in 0..16u64 {
            let nonce = now_ns().wrapping_add(attempt);
            let mut hasher = Sha256::new();
            hasher.update(command);
            hasher.update([0]);
            hasher.update(exit_code.to_string());
            hasher.update([0]);
            hasher.update(raw);
            hasher.update(nonce.to_string());
            let hex = format!("{:x}", hasher.finalize());
            let rf = format!("cmdout_{}_{nonce:016x}", &hex[..12]);
            let path = self.path_for(&rf)?;
            let raw_text = std::str::from_utf8(raw).ok();
            let payload = serde_json::json!({
                "command":command,
                "exit_code":exit_code,
                "raw":raw_text,
                "raw_b64":BASE64_STANDARD.encode(raw),
                "created_ns":nonce
            });
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            options.mode(0o600);
            match options.open(&path) {
                Ok(mut f) => {
                    f.write_all(serde_json::to_string_pretty(&payload)?.as_bytes())?;
                    return Ok(rf);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        bail!("could not allocate raw_ref")
    }
    fn path_for(&self, raw_ref: &str) -> Result<PathBuf> {
        let re = Regex::new(r"^cmdout_[0-9a-f]{12}_[0-9a-f]{16}$").expect("valid raw_ref regex");
        if !re.is_match(raw_ref) {
            bail!("invalid raw ref: {raw_ref}");
        }
        let root = self.root.canonicalize().or_else(|_| {
            fs::create_dir_all(&self.root)?;
            self.root.canonicalize()
        })?;
        let path = root.join(format!("{raw_ref}.json"));
        let Some(parent_path) = path.parent() else {
            bail!("raw ref path has no parent");
        };
        let parent = parent_path.canonicalize()?;
        if parent != root {
            bail!("raw ref escapes store");
        }
        Ok(path)
    }
    pub fn raw(&self, raw_ref: &str, around: Option<&str>, context: usize) -> Result<String> {
        let raw_bytes = self.raw_bytes(raw_ref, around, context)?;
        Ok(String::from_utf8_lossy(&raw_bytes).to_string())
    }
    pub fn raw_bytes(
        &self,
        raw_ref: &str,
        around: Option<&str>,
        context: usize,
    ) -> Result<Vec<u8>> {
        let path = self.path_for(raw_ref)?;
        let text = fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;
        let raw = if let Some(raw_b64) = v["raw_b64"].as_str() {
            BASE64_STANDARD.decode(raw_b64)?
        } else {
            v["raw"].as_str().unwrap_or("").as_bytes().to_vec()
        };
        if let Some(needle) = around {
            let raw_text = std::str::from_utf8(&raw)?;
            let lines: Vec<_> = raw_text.lines().collect();
            let mut out = Vec::new();
            for (i, _l) in lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.contains(needle))
                .take(5)
            {
                let start = i.saturating_sub(context);
                let end = (i + context + 1).min(lines.len());
                out.extend_from_slice(&lines[start..end]);
            }
            if !out.is_empty() {
                return Ok((out.join("\n") + "\n").into_bytes());
            }
            bail!("raw range needle not found: {needle}");
        }
        Ok(raw)
    }
}

pub fn run_command(
    command: &[String],
    cwd: Option<&Path>,
    raw_dir: impl AsRef<Path>,
    max_summary_bytes: usize,
) -> Result<CommandSummary> {
    run_command_with_rules(command, cwd, raw_dir, max_summary_bytes, None)
}

pub fn run_command_with_rules(
    command: &[String],
    cwd: Option<&Path>,
    raw_dir: impl AsRef<Path>,
    max_summary_bytes: usize,
    rules: Option<&CommandRuleSet>,
) -> Result<CommandSummary> {
    let store = RawStore::new(raw_dir)?;
    if command.is_empty() {
        return compress_with_rules(
            &store,
            &[],
            "",
            "[tfy: command launch failed] empty command\n",
            127,
            Some(max_summary_bytes),
            ToolPolicy::Auto,
            rules,
        );
    }
    let mut cmd = Command::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(out) => {
            let mut raw = out.stdout;
            raw.extend_from_slice(&out.stderr);
            compress_bytes(
                &store,
                &command.join(" "),
                Some(command),
                &raw,
                out.status.code().unwrap_or(-1),
                Some(max_summary_bytes),
                ToolPolicy::Auto,
                rules,
            )
        }
        Err(e) => compress_with_rules(
            &store,
            command,
            &command.join(" "),
            &format!(
                "[tfy: command launch failed] {}: {}\n",
                std::any::type_name_of_val(&e),
                e
            ),
            127,
            Some(max_summary_bytes),
            ToolPolicy::Auto,
            rules,
        ),
    }
}

pub fn raw_output(
    raw_dir: impl AsRef<Path>,
    raw_ref: &str,
    around: Option<&str>,
    context: usize,
) -> Result<String> {
    RawStore::new(raw_dir)?.raw(raw_ref, around, context)
}

pub fn raw_output_bytes(
    raw_dir: impl AsRef<Path>,
    raw_ref: &str,
    around: Option<&str>,
    context: usize,
) -> Result<Vec<u8>> {
    RawStore::new(raw_dir)?.raw_bytes(raw_ref, around, context)
}

pub fn summarize_command_output(
    command: &str,
    raw: &str,
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
) -> Result<CommandSummary> {
    summarize_command_output_with_policy(command, raw, exit_code, raw_dir, ToolPolicy::Auto)
}

pub fn summarize_command_output_with_policy(
    command: &str,
    raw: &str,
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
    policy: ToolPolicy,
) -> Result<CommandSummary> {
    let store = RawStore::new(raw_dir)?;
    compress_with_rules(&store, &[], command, raw, exit_code, None, policy, None)
}

pub fn summarize_command_output_with_rules(
    command: &str,
    argv: &[String],
    raw: &str,
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
    rules: Option<&CommandRuleSet>,
) -> Result<CommandSummary> {
    summarize_command_output_bytes_with_rules(
        command,
        argv,
        raw.as_bytes(),
        exit_code,
        raw_dir,
        rules,
    )
}

pub fn summarize_command_output_bytes_with_rules(
    command: &str,
    argv: &[String],
    raw_bytes: &[u8],
    exit_code: i32,
    raw_dir: impl AsRef<Path>,
    rules: Option<&CommandRuleSet>,
) -> Result<CommandSummary> {
    let store = RawStore::new(raw_dir)?;
    compress_bytes(
        &store,
        command,
        Some(argv),
        raw_bytes,
        exit_code,
        None,
        ToolPolicy::Auto,
        rules,
    )
}

#[allow(clippy::too_many_arguments)]
fn compress_with_rules(
    store: &RawStore,
    argv: &[String],
    command: &str,
    raw: &str,
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
    rules: Option<&CommandRuleSet>,
) -> Result<CommandSummary> {
    compress_bytes(
        store,
        command,
        Some(argv),
        raw.as_bytes(),
        exit_code,
        max_summary_bytes,
        requested_policy,
        rules,
    )
}

#[allow(clippy::too_many_arguments)]
fn compress_bytes(
    store: &RawStore,
    command: &str,
    argv: Option<&[String]>,
    raw_bytes: &[u8],
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
    rules: Option<&CommandRuleSet>,
) -> Result<CommandSummary> {
    let raw_ref = store.put_bytes(command, raw_bytes, exit_code)?;
    let raw = String::from_utf8_lossy(raw_bytes);
    compress_with_raw_ref(
        command,
        argv.unwrap_or(&[]),
        &raw,
        raw_bytes.len(),
        exit_code,
        max_summary_bytes,
        requested_policy,
        rules,
        StoredRaw {
            raw_ref,
            output_sha256: sha256_hex(raw_bytes),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn compress_with_raw_ref(
    command: &str,
    argv: &[String],
    raw: &str,
    raw_len: usize,
    exit_code: i32,
    max_summary_bytes: Option<usize>,
    requested_policy: ToolPolicy,
    rules: Option<&CommandRuleSet>,
    stored_raw: StoredRaw,
) -> Result<CommandSummary> {
    let raw_ref = stored_raw.raw_ref;
    let output_sha256 = stored_raw.output_sha256;
    let policy = GitGithubToolPolicy::new();
    let registry = CommandStrategyRegistry;
    let strategy_metadata = registry.classify(command);
    let command_family = strategy_metadata.family.clone();
    let is_git_github = match requested_policy {
        ToolPolicy::Auto => policy.matches(command),
        ToolPolicy::Generic => false,
        ToolPolicy::GitGithub => true,
    };
    let evidence = if is_git_github {
        policy.evidence(command, raw)
    } else {
        evidence(raw)
    }
    .into_iter()
    .map(|line| redact_public(&line))
    .collect::<Vec<_>>();
    let risk = family_risk(
        &command_family,
        command,
        raw,
        exit_code,
        is_git_github,
        &policy,
    );
    let display_command = redact_public(command);
    let mut strategy_kind = strategy_metadata.strategy_kind;
    let mut human_auto_safe = strategy_metadata.human_auto_safe;
    let mut agent_safe = strategy_metadata.agent_safe;
    let mut interactive_risk = strategy_metadata.interactive_risk;
    let mut rule_id = None;
    let mut strategy_source_kind = if strategy_kind == "generic" {
        "none".to_string()
    } else {
        "built_in".to_string()
    };
    let mut command_rule_diagnostics = rules
        .map(|rules| rules.diagnostics().to_vec())
        .unwrap_or_default();
    let built_in_candidate = registry.summary_candidate(StrategySummaryInput {
        family: &command_family,
        cmd: &display_command,
        code: exit_code,
        raw,
        evidence: &evidence,
        rr: &raw_ref,
        risk: &risk,
    });
    let user_candidate = rules.and_then(|rules| {
        rules.summary_candidate(
            argv,
            &display_command,
            exit_code,
            raw,
            &evidence,
            &raw_ref,
            &risk,
        )
    });
    let use_user_candidate = match (&built_in_candidate, &user_candidate) {
        (Some(_), Some(user_candidate))
            if user_candidate
                .override_policy
                .allows_family(&command_family) =>
        {
            let reason = user_candidate
                .override_policy
                .reason
                .as_deref()
                .map(|reason| format!("; reason={reason}"))
                .unwrap_or_default();
            command_rule_diagnostics.push(CommandRuleDiagnostic {
                source_kind: user_candidate.strategy_source_kind.clone(),
                path: String::new(),
                code: "user_rule_overrode_builtin".into(),
                message: format!(
                    "user command rule {} explicitly overrode built-in family {command_family}{reason}",
                    user_candidate.rule_id
                ),
            });
            true
        }
        (Some(_), Some(user_candidate)) if user_candidate.override_policy.built_in => {
            let requested = user_candidate
                .override_policy
                .family
                .as_deref()
                .unwrap_or("<missing>");
            command_rule_diagnostics.push(CommandRuleDiagnostic {
                source_kind: user_candidate.strategy_source_kind.clone(),
                path: String::new(),
                code: "user_rule_override_family_mismatch".into(),
                message: format!(
                    "user command rule {} requested built-in override family {requested}, but command classified as {command_family}; built-in strategy {strategy_kind} kept precedence",
                    user_candidate.rule_id
                ),
            });
            false
        }
        (Some(_), Some(user_candidate)) => {
            command_rule_diagnostics.push(CommandRuleDiagnostic {
                source_kind: user_candidate.strategy_source_kind.clone(),
                path: String::new(),
                code: "user_rule_shadowed_by_builtin".into(),
                message: format!(
                    "user command rule {} matched but built-in strategy {strategy_kind} kept precedence",
                    user_candidate.rule_id
                ),
            });
            false
        }
        (Some(_), None) => {
            if let Some(rule_set) = rules {
                if let Some(rule) = rule_set.first_match(argv, &display_command) {
                    command_rule_diagnostics.push(CommandRuleDiagnostic {
                        source_kind: rule.source_kind.clone(),
                        path: String::new(),
                        code: "user_rule_shadowed_by_builtin".into(),
                        message: format!(
                            "user command rule {} matched but produced no summary candidate; built-in strategy {strategy_kind} kept precedence",
                            rule.id
                        ),
                    });
                }
            }
            false
        }
        (None, Some(_)) => true,
        (None, None) => false,
    };
    let summary_candidate = if use_user_candidate {
        let user_candidate = user_candidate.expect("user candidate exists when selected");
        strategy_kind = "user_toml".into();
        human_auto_safe = user_candidate.human_auto_safe;
        agent_safe = user_candidate.agent_safe;
        interactive_risk = user_candidate.interactive_risk;
        rule_id = Some(user_candidate.rule_id);
        strategy_source_kind = user_candidate.strategy_source_kind;
        user_candidate.text
    } else if let Some(built_in_candidate) = built_in_candidate {
        built_in_candidate
    } else {
        match risk.as_str() {
            "critical" => critical(&display_command, exit_code, &evidence, &raw_ref),
            "success" => success(&display_command, exit_code, raw, &raw_ref),
            _ => unknown(&display_command, exit_code, raw, &evidence, &raw_ref),
        }
    };
    let summary_candidate =
        cap_summary_preserving_raw_ref(summary_candidate, &raw_ref, max_summary_bytes);
    let public_raw = public_raw_candidate(raw, &raw_ref);
    let decision = choose_model_visible_text(&summary_candidate, &public_raw);
    let savings_pct = savings_pct_floor(raw_len, decision.text.len());
    Ok(CommandSummary {
        command: display_command,
        exit_code,
        risk,
        summary_chars: decision.text.len(),
        raw_chars: raw_len,
        savings_pct,
        summary: decision.text.clone(),
        model_text: decision.text,
        rendering_kind: decision.kind,
        raw_ref,
        evidence,
        command_family,
        strategy_kind,
        human_auto_safe,
        agent_safe,
        interactive_risk,
        rule_id,
        strategy_source_kind,
        command_rule_diagnostics,
        output_sha256,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn savings_pct_floor(raw_len: usize, model_len: usize) -> f64 {
    if raw_len == 0 || model_len >= raw_len {
        0.0
    } else {
        ((raw_len as f64 - model_len as f64) / raw_len as f64 * 10000.0).round() / 100.0
    }
}

struct ModelOutputDecision {
    text: String,
    kind: String,
}

fn choose_model_visible_text(
    summary_candidate: &str,
    public_raw: &PublicRawCandidate,
) -> ModelOutputDecision {
    if summary_candidate.len() < public_raw.text.len() {
        ModelOutputDecision {
            text: summary_candidate.to_string(),
            kind: "summary".to_string(),
        }
    } else if public_raw.suppressed {
        ModelOutputDecision {
            text: public_raw.text.clone(),
            kind: "suppressed".to_string(),
        }
    } else {
        ModelOutputDecision {
            text: public_raw.text.clone(),
            kind: "pass_through".to_string(),
        }
    }
}

struct PublicRawCandidate {
    text: String,
    suppressed: bool,
}

fn public_raw_candidate(raw: &str, raw_ref: &str) -> PublicRawCandidate {
    if is_suppressed_public_raw(raw) {
        return PublicRawCandidate {
            text: format!(
                "[tfy: output suppressed; unsafe or binary-ish content stored locally; raw_ref={raw_ref}]\n"
            ),
            suppressed: true,
        };
    }
    PublicRawCandidate {
        text: redact_public(raw),
        suppressed: false,
    }
}

fn is_suppressed_public_raw(raw: &str) -> bool {
    raw.chars()
        .any(|c| (c.is_control() && !matches!(c, '\n' | '\r' | '\t')) || c == '\u{fffd}')
}

struct GitGithubToolPolicy {
    command: Regex,
    git_status: Regex,
    git_data: Regex,
    gh_review_data: Regex,
    git_visible_data: Regex,
    git_error: Regex,
    negative: Regex,
    file_line: Regex,
    diff_hunk: Regex,
    url: Regex,
}

impl GitGithubToolPolicy {
    fn new() -> Self {
        let generic = r"error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL";
        let git = format!(
            r"{generic}|conflict|rejected|non-fast-forward|rate limit|authenticate|HTTP\s+[45]\d\d|changes_requested|unresolved"
        );
        Self {
            command: Regex::new(r"^(?:git|gh)(?:\s|$)").expect("valid command regex"),
            git_status: Regex::new(r"^git\s+status(?:\s|$)").expect("valid git status regex"),
            git_data: Regex::new(r"^git\s+(?:diff|show|log)(?:\s|$)").expect("valid git data regex"),
            gh_review_data: Regex::new(r"^gh\s+(?:pr|issue)\s+(?:view|status)(?:\s|$)").expect("valid gh review regex"),
            git_visible_data: Regex::new(r"^git\s+(?:diff|show|log|status)(?:\s|$)").expect("valid git visible regex"),
            git_error: Regex::new(&format!(r"(?i)({git})")).expect("valid git error regex"),
            negative: Regex::new(r"(?i)(not\s+successful|unsuccessful|failed|failing|cancel(?:led|ed)|timed?\s+out|action_required)").expect("valid negative regex"),
            file_line: Regex::new(r"[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md)(?::\d+)?").expect("valid file regex"),
            diff_hunk: Regex::new(r"^@@\s.*@@").expect("valid diff hunk regex"),
            url: Regex::new(r#"https?://[^\s\"'<>]+"#).expect("valid url regex"),
        }
    }

    fn matches(&self, command: &str) -> bool {
        self.command.is_match(command.trim())
    }

    fn risk(&self, command: &str, raw: &str, exit_code: i32) -> String {
        let output = raw.trim_matches(['\r', '\n']);
        if exit_code != 0 {
            return "critical".into();
        }
        if self.is_git_status(command) {
            if self.is_clean_git_status(output) {
                return "success".into();
            }
            if self.has_git_status_conflict_marker(output)
                || self.has_long_git_status_blocker(output)
            {
                return "critical".into();
            }
            return if output.is_empty() {
                "success"
            } else {
                "unknown"
            }
            .into();
        }
        if (self.is_git_data(command) || self.is_gh_review_data(command)) && !output.is_empty() {
            return "unknown".into();
        }
        if self.git_error.is_match(raw) || self.negative.is_match(raw) {
            return "critical".into();
        }
        if self.is_git_github_success(command, raw) {
            return "success".into();
        }
        "unknown".into()
    }

    fn is_git_status(&self, command: &str) -> bool {
        self.git_status.is_match(command.trim())
    }
    fn is_git_data(&self, command: &str) -> bool {
        self.git_data.is_match(command.trim())
    }
    fn is_gh_review_data(&self, command: &str) -> bool {
        self.gh_review_data.is_match(command.trim())
    }
    fn is_git_github_success(&self, command: &str, raw: &str) -> bool {
        let text = raw.trim();
        if text.is_empty() {
            return true;
        }
        if self.is_git_status(command) {
            return self.is_clean_git_status(text);
        }
        if self.git_visible_data.is_match(command.trim()) || self.negative.is_match(text) {
            return false;
        }
        Regex::new(
            r"(?i)all checks passed|checks? passed|success(?:ful)?|passed|up[ -]?to[ -]?date",
        )
        .expect("valid git success regex")
        .is_match(text)
    }
    fn is_clean_git_status(&self, text: &str) -> bool {
        if text.is_empty() {
            return true;
        }
        let normalized = text
            .trim()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        let lines: Vec<_> = normalized
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        if !lines.is_empty() && lines.iter().all(|line| line.starts_with("##")) {
            return true;
        }
        if matches!(
            normalized.as_str(),
            "nothing to commit, working tree clean" | "nothing to commit, working tree clean."
        ) {
            return true;
        }
        Regex::new(r"(?is)^On branch .+(?:\nYour branch is up to date with .+\.)?\n\nnothing to commit, working tree clean\.?$")
            .expect("valid clean status regex")
            .is_match(&normalized)
    }
    fn looks_like_git_status_line(&self, line: &str) -> bool {
        if line.starts_with("##") {
            return true;
        }
        let bytes = line.as_bytes();
        bytes.len() >= 3
            && b" MADRCUT?!".contains(&bytes[0])
            && b" MADRCUT?!".contains(&bytes[1])
            && bytes[2].is_ascii_whitespace()
    }
    fn has_git_status_conflict_marker(&self, text: &str) -> bool {
        text.lines().any(|line| {
            Regex::new(r"^(?:UU|AA|DD|AU|UA|DU|UD)(?:\s|$)")
                .expect("valid conflict marker regex")
                .is_match(line.trim())
        })
    }
    fn has_long_git_status_blocker(&self, text: &str) -> bool {
        text.lines()
            .any(|line| self.is_long_git_status_blocker_line(line))
    }
    fn is_long_git_status_blocker_line(&self, line: &str) -> bool {
        Regex::new(
            r"(?i)^(?:unmerged paths:|conflict\b|rejected\b|authentication\b|fatal\b|error\b)",
        )
        .expect("valid blocker regex")
        .is_match(line.trim())
    }
    fn git_status_line_evidence(&self, line: &str) -> Option<String> {
        if line.starts_with("##") || !self.looks_like_git_status_line(line) {
            return None;
        }
        let path = if line.len() > 3 { line[3..].trim() } else { "" };
        if path.is_empty() {
            Some(format!("status={}", &line[..2]))
        } else {
            Some(format!("status={} path={path}", &line[..2]))
        }
    }
    fn evidence(&self, command: &str, raw: &str) -> Vec<String> {
        let is_git_status = self.is_git_status(command);
        let is_git_data = self.is_git_data(command);
        let mut error_lines = Vec::new();
        let mut file_lines = Vec::new();
        let mut hunk_lines = Vec::new();
        let mut url_lines = Vec::new();
        for line in raw.lines() {
            let evidence_line = self.redact_for_summary(line);
            let status_evidence = if is_git_status {
                self.git_status_line_evidence(&evidence_line)
            } else {
                None
            };
            let blocker_line =
                is_git_status && self.is_long_git_status_blocker_line(&evidence_line);
            let error_match = if is_git_data {
                None
            } else {
                self.git_error.find(&evidence_line)
            };
            let file_match = self.file_line.find(&evidence_line);
            let hunk_match = self.diff_hunk.find(&evidence_line);
            let url_match = self.url.find(&evidence_line);
            let negative_match = if is_git_data {
                None
            } else {
                self.negative.find(&evidence_line)
            };
            if blocker_line {
                push(&mut error_lines, evidence_line.trim().to_string());
            } else if let Some(status) = status_evidence {
                push(&mut file_lines, status);
            } else if let (Some(f), Some(e)) = (file_match, error_match) {
                push(&mut error_lines, excerpt_multi(&evidence_line, &[f, e]));
            } else if let Some(e) = error_match {
                push(
                    &mut error_lines,
                    excerpt(&evidence_line, e.start(), e.end()),
                );
            } else if let Some(n) = negative_match {
                push(
                    &mut error_lines,
                    excerpt(&evidence_line, n.start(), n.end()),
                );
            } else if let Some(u) = url_match {
                push(&mut url_lines, excerpt(&evidence_line, u.start(), u.end()));
            } else if let Some(h) = hunk_match {
                push(&mut hunk_lines, excerpt(&evidence_line, h.start(), h.end()));
            } else if let Some(f) = file_match {
                push(&mut file_lines, excerpt(&evidence_line, f.start(), f.end()));
            }
        }
        error_lines
            .into_iter()
            .chain(url_lines)
            .chain(file_lines)
            .chain(hunk_lines)
            .take(12)
            .collect()
    }
    fn redact_for_summary(&self, text: &str) -> String {
        self.url
            .replace_all(text, |caps: &regex::Captures| {
                redact_url(caps.get(0).unwrap().as_str())
            })
            .to_string()
    }
}

/// Return the stable command-family label used by Tool Gateway execution and adapter reports.
///
/// The classifier is intentionally lightweight: cheap argv/text dispatch only. Unknown or
/// shell-wrapper commands fall back to `generic` rather than weakening evidence handling.
pub fn classify_command_family(command: &str) -> String {
    let cmd = command.trim();
    let words: Vec<&str> = cmd.split_whitespace().collect();
    if words.is_empty() {
        return "generic".into();
    }
    match words.as_slice() {
        ["sh", "-c", rest @ ..] | ["bash", "-lc", rest @ ..] | ["bash", "-c", rest @ ..] => {
            // Preserve real wrapper classification for analytics unless the wrapped command
            // starts with a P0 command. This keeps test/demo wrappers useful without claiming
            // universal shell interception.
            classify_wrapped_shell_command(&rest.join(" "))
        }
        _ => classify_direct_command(cmd),
    }
}

fn classify_wrapped_shell_command(command: &str) -> String {
    let first = command.split([';', '|']).next().unwrap_or(command).trim();
    let first = first
        .strip_prefix("exec ")
        .unwrap_or(first)
        .strip_prefix("env ")
        .unwrap_or(first)
        .trim();
    classify_direct_command(first)
}

fn classify_direct_command(cmd: &str) -> String {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    match words.as_slice() {
        ["git", "status", ..] => "git_status".into(),
        ["git", "diff", ..] => "git_diff".into(),
        ["git", "log", ..] => "git_log".into(),
        ["gh", "pr", "checks", ..] => "gh_pr_checks".into(),
        ["cargo", "test", ..] => "cargo_test".into(),
        ["cargo", "clippy", ..] => "cargo_clippy".into(),
        ["cargo", "build", ..] => "cargo_build".into(),
        ["cargo", "check", ..] => "cargo_check".into(),
        ["cargo", "fmt", rest @ ..] if args_contain_exact(rest, "--check") => {
            "cargo_fmt_check".into()
        }
        ["pytest", ..] | ["python", "-m", "pytest", ..] | ["python3", "-m", "pytest", ..] => {
            "pytest".into()
        }
        ["npm", "test", ..] => "npm_test".into(),
        ["pnpm", "test", ..] => "pnpm_test".into(),
        ["yarn", "test", ..] => "yarn_test".into(),
        ["npm", "install", ..] | ["npm", "ci", ..] => "npm_install".into(),
        ["pnpm", "install", ..] => "pnpm_install".into(),
        ["yarn", "install", ..] => "yarn_install".into(),
        ["npx", "vitest", ..] | ["vitest", ..] => "vitest".into(),
        ["next", "build", ..] | ["npx", "next", "build", ..] => "next".into(),
        ["eslint", ..] | ["npx", "eslint", ..] => "lint".into(),
        ["prettier", ..] | ["npx", "prettier", ..] => "prettier".into(),
        ["playwright", ..] | ["npx", "playwright", ..] => "playwright".into(),
        ["prisma", ..] | ["npx", "prisma", ..] => "prisma".into(),
        ["biome", ..] | ["npx", "biome", ..] => "biome".into(),
        ["turbo", ..] | ["npx", "turbo", ..] => "turbo".into(),
        ["nx", ..] | ["npx", "nx", ..] => "nx".into(),
        ["go", "test", ..] => "go_test".into(),
        ["golangci-lint", ..] => "golangci-lint".into(),
        ["df", ..] => "df".into(),
        ["du", ..] => "du".into(),
        ["find", ..] => "find".into(),
        ["grep", ..] | ["rg", ..] => "grep".into(),
        ["wc", ..] => "wc".into(),
        ["env", ..] | ["printenv", ..] => "env".into(),
        ["jq", ..] => "jq".into(),
        ["ps", ..] => "ps".into(),
        ["make", ..] => "make".into(),
        ["just", ..] => "just".into(),
        ["shellcheck", ..] => "shellcheck".into(),
        ["pre-commit", ..] => "pre-commit".into(),
        ["ruff", ..] => "ruff".into(),
        ["mypy", ..] => "mypy".into(),
        ["pip", "install", ..]
        | ["python", "-m", "pip", "install", ..]
        | ["python3", "-m", "pip", "install", ..] => "pip".into(),
        ["uv", "sync", ..] => "uv-sync".into(),
        ["poetry", "install", ..] => "poetry-install".into(),
        ["rspec", ..] => "rspec".into(),
        ["rubocop", ..] => "rubocop".into(),
        ["bundle", "install", ..] => "bundle-install".into(),
        ["dotnet", "build", ..] => "dotnet-build".into(),
        ["dotnet", "test", ..] => "dotnet".into(),
        ["terraform", "plan", ..] => "terraform-plan".into(),
        ["tofu", "plan", ..] => "tofu-plan".into(),
        ["helm", ..] => "helm".into(),
        ["kubectl", ..] => "kubectl".into(),
        ["docker", ..] => "docker".into(),
        ["aws", ..] => "aws".into(),
        ["gcloud", ..] => "gcloud".into(),
        ["systemctl", "status", ..] => "systemctl-status".into(),
        ["mvn", rest @ ..] | ["mvnw", rest @ ..] | ["./mvnw", rest @ ..]
            if java_test_args(rest) =>
        {
            "maven_test".into()
        }
        ["gradle", rest @ ..] | ["gradlew", rest @ ..] | ["./gradlew", rest @ ..]
            if java_test_args(rest) =>
        {
            "gradle_test".into()
        }
        ["tsc", rest @ ..]
        | ["npx", "tsc", rest @ ..]
        | ["pnpm", "exec", "tsc", rest @ ..]
        | ["yarn", "tsc", rest @ ..]
        | ["npm", "exec", "tsc", rest @ ..]
            if tsc_no_emit_args(rest) =>
        {
            "tsc_check".into()
        }
        _ => "generic".into(),
    }
}

fn args_contain_exact(args: &[&str], needle: &str) -> bool {
    args.contains(&needle)
}

fn tsc_no_emit_args(args: &[&str]) -> bool {
    args_contain_exact(args, "--noEmit") && !tsc_long_running_args(args)
}

fn java_test_args(args: &[&str]) -> bool {
    args.iter().any(|arg| {
        matches!(
            *arg,
            "test"
                | "verify"
                | "check"
                | "build"
                | ":test"
                | "testDebugUnitTest"
                | "connectedAndroidTest"
        ) || arg.ends_with(":test")
    })
}

fn tsc_long_running_args(args: &[&str]) -> bool {
    args.iter()
        .any(|arg| matches!(*arg, "--watch" | "-w" | "--watchFile" | "--watchDirectory"))
}

fn family_risk(
    family: &str,
    command: &str,
    raw: &str,
    exit_code: i32,
    is_git_github: bool,
    policy: &GitGithubToolPolicy,
) -> String {
    if exit_code != 0 {
        return "critical".into();
    }
    match family {
        "cargo_test" | "pytest" | "npm_test" | "pnpm_test" | "yarn_test" | "go_test"
        | "maven_test" | "gradle_test" => {
            if has_nonzero_test_failures(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() || has_zero_test_failures(raw) {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "gh_pr_checks" => {
            if has_nonzero_check_failures(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if Regex::new(r"(?i)(all checks passed|checks? passed|pass(?:ed|ing)?|success)")
                .expect("valid checks success regex")
                .is_match(raw)
            {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "cargo_clippy" | "cargo_build" | "cargo_check" => {
            if has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "cargo_fmt_check" => {
            if has_cargo_fmt_diff(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        "tsc_check" => {
            if has_typescript_diagnostics(raw) || has_hard_failure_marker(raw) {
                "critical".into()
            } else if is_success(raw) || raw.trim().is_empty() {
                "success".into()
            } else {
                "unknown".into()
            }
        }
        _ if built_in_filter_for_family(family)
            .map(|filter| filter.is_critical(raw))
            .unwrap_or(false) =>
        {
            "critical".into()
        }
        _ if built_in_filter_for_family(family).is_some()
            && (is_success(raw) || !raw.trim().is_empty()) =>
        {
            "success".into()
        }
        _ if is_git_github => policy.risk(command, raw, exit_code),
        _ if is_error(raw) => "critical".into(),
        _ if is_success(raw) || raw.trim().is_empty() => "success".into(),
        _ => "unknown".into(),
    }
}

fn has_cargo_fmt_diff(raw: &str) -> bool {
    raw.lines().any(|line| line.starts_with("Diff in "))
}

fn has_typescript_diagnostics(raw: &str) -> bool {
    Regex::new(r"(?m)\berror\s+TS[0-9]+:")
        .expect("valid TypeScript diagnostic regex")
        .is_match(raw)
}

fn has_zero_test_failures(raw: &str) -> bool {
    Regex::new(r"(?i)\b0\s+(?:failed|failures?|failing)\b")
        .expect("valid zero test failures regex")
        .is_match(raw)
}

fn has_nonzero_test_failures(raw: &str) -> bool {
    Regex::new(r"(?i)\b[1-9][0-9]*\s+(?:failed|failures?|failing)\b")
        .expect("valid nonzero test failures regex")
        .is_match(raw)
}

fn has_nonzero_check_failures(raw: &str) -> bool {
    let numeric_failure = Regex::new(
        r"(?i)\b[1-9][0-9]*\s+(?:failed|failing|cancel(?:led|ed)|timed? out|action_required)\b",
    )
    .expect("valid nonzero check failures regex");
    if numeric_failure.is_match(raw) {
        return true;
    }

    let zero_failure =
        Regex::new(r"(?i)\b0\s+(?:failed|failing)\b").expect("valid zero check failure regex");
    let failure_marker =
        Regex::new(r"(?i)\b(fail(?:ed|ing)?|error|cancel(?:led|ed)|timed? out|action_required)\b")
            .expect("valid check failure marker regex");
    raw.lines()
        .any(|line| failure_marker.is_match(line) && !zero_failure.is_match(line))
}

fn has_hard_failure_marker(raw: &str) -> bool {
    Regex::new(r"(?i)(panic|traceback|exception|fatal|denied|unauthori[sz]ed|forbidden|permission\s+denied|token\s+expired|assertion failed|not\s+successful|unsuccessful)")
        .expect("valid hard failure regex")
        .is_match(raw)
}

#[allow(dead_code)]
struct BuiltInFilterFixture {
    name: &'static str,
    raw: &'static str,
    exit_code: i32,
    expect_contains: &'static [&'static str],
}

struct BuiltInFilter {
    family: &'static str,
    strip_ansi: bool,
    replace: &'static [(&'static str, &'static str)],
    match_output: &'static [(&'static str, &'static [&'static str])],
    strip_lines_matching: &'static [&'static str],
    keep_lines_matching: &'static [&'static str],
    preserve_lines_matching: &'static [&'static str],
    unsafe_if_matches: &'static [&'static str],
    truncate_lines_at: usize,
    head_lines: usize,
    tail_lines: usize,
    max_lines: usize,
    on_empty: &'static str,
    human_auto_safe: bool,
    interactive_risk: &'static str,
    #[allow(dead_code)]
    fixtures: &'static [BuiltInFilterFixture],
}

const DEFAULT_DSL_FIXTURES: &[BuiltInFilterFixture] = &[BuiltInFilterFixture {
    name: "default-diagnostic",
    raw: "setup ok\nwarning: deprecated flag\nsrc/app.rs:42: error: example failure\ncompleted with diagnostics\n",
    exit_code: 0,
    expect_contains: &["warning", "src/app.rs:42", "raw_ref="],
}];

const DF_FIXTURES: &[BuiltInFilterFixture] = &[BuiltInFilterFixture {
    name: "df-high-usage",
    raw: "Filesystem      Size  Used Avail Use% Mounted on\n/dev/disk1s1    100G   95G    5G  95% /\n/dev/disk2s1    200G   20G  180G  10% /data\n",
    exit_code: 0,
    expect_contains: &["family=df", "/dev/disk1s1", "95%", "raw_ref="],
}];

macro_rules! tfy_filter {
    ($family:literal, human_safe = $human_safe:expr, risk = $risk:literal, fixtures = $fixtures:expr) => {
        BuiltInFilter {
            family: $family,
            strip_ansi: true,
            replace: &[],
            match_output: &[],
            strip_lines_matching: &[r"(?i)^(?:\s*\[?debug\]?|\s*trace:)"],
            keep_lines_matching: &[],
            preserve_lines_matching: &[
                r"(?i)(error|failed|failure|fatal|panic|warning|denied|unauthori[sz]ed|forbidden|deprecated)",
                r"(?i)(\b[A-Za-z0-9_./-]+\.(?:rs|ts|tsx|js|jsx|py|rb|go|java|kt|cs|json|ya?ml|toml|tf):[0-9]+)",
                r"(?i)(\b[8-9][0-9]%|100%)",
            ],
            unsafe_if_matches: &[
                r"(?i)(error|failed|failure|fatal|panic|denied|unauthori[sz]ed|forbidden|100%)",
            ],
            truncate_lines_at: 180,
            head_lines: 16,
            tail_lines: 8,
            max_lines: 32,
            on_empty: concat!($family, ": no relevant output"),
            human_auto_safe: $human_safe,
            interactive_risk: $risk,
            fixtures: $fixtures,
        }
    };
}

const BUILT_IN_FILTERS: &[BuiltInFilter] = &[
    tfy_filter!(
        "df",
        human_safe = true,
        risk = "none",
        fixtures = DF_FIXTURES
    ),
    tfy_filter!(
        "du",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "find",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "grep",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "wc",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "env",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "jq",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "ps",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "make",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "just",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "shellcheck",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pre-commit",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "npm_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pnpm_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "yarn_install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "vitest",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "next",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "lint",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "prettier",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "playwright",
        human_safe = true,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "prisma",
        human_safe = true,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "biome",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "turbo",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "nx",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "ruff",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "mypy",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "pip",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "uv-sync",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "poetry-install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "rspec",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "rubocop",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "bundle-install",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "golangci-lint",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "dotnet-build",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "dotnet",
        human_safe = true,
        risk = "none",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "terraform-plan",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "tofu-plan",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "helm",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "kubectl",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "docker",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "aws",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "gcloud",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
    tfy_filter!(
        "systemctl-status",
        human_safe = false,
        risk = "possible",
        fixtures = DEFAULT_DSL_FIXTURES
    ),
];

fn built_in_filter_for_family(family: &str) -> Option<&'static BuiltInFilter> {
    BUILT_IN_FILTERS
        .iter()
        .find(|filter| filter.family == family)
}

fn built_in_filter_summary_candidate(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> Option<String> {
    let filter = built_in_filter_for_family(family)?;
    let filtered = apply_built_in_filter(filter, raw)?;
    let mut lines = vec![format!(
        "TFY command summary: {} family={} strategy=dsl exit={code} cmd={cmd}",
        risk.to_uppercase(),
        filter.family
    )];
    lines.push(format!(
        "- selected_lines={} original_lines={}",
        filtered.selected_lines,
        raw.lines().count()
    ));
    for line in filtered.preserved.iter().take(8) {
        lines.push(format!("- preserved: {line}"));
    }
    for line in filtered.lines.iter().take(filter.max_lines) {
        lines.push(format!("- {line}"));
    }
    if risk != "success" {
        lines.extend(evidence.iter().take(5).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    Some(lines.join("\n"))
}

struct FilteredOutput {
    lines: Vec<String>,
    preserved: Vec<String>,
    selected_lines: usize,
}

fn apply_built_in_filter(filter: &BuiltInFilter, raw: &str) -> Option<FilteredOutput> {
    let mut text = if filter.strip_ansi {
        strip_ansi_sequences(raw)
    } else {
        raw.to_string()
    };
    for (pattern, replacement) in filter.replace {
        let re = Regex::new(pattern).expect("valid built-in filter replace regex");
        text = re.replace_all(&text, *replacement).to_string();
    }
    for (pattern, unless_patterns) in filter.match_output {
        let re = Regex::new(pattern).expect("valid built-in filter match_output regex");
        let blocked = unless_patterns.iter().any(|unless| {
            Regex::new(unless)
                .expect("valid built-in filter unless regex")
                .is_match(&text)
        });
        if !re.is_match(&text) || blocked {
            return None;
        }
    }
    let strip_res = compile_regexes(filter.strip_lines_matching);
    let keep_res = compile_regexes(filter.keep_lines_matching);
    let preserve_res = compile_regexes(filter.preserve_lines_matching);
    let mut lines = Vec::new();
    let mut preserved = Vec::new();
    for line in text.lines() {
        let redacted = redact_public(line);
        let normalized = cap_to_chars(&norm(&redacted), filter.truncate_lines_at);
        if normalized.is_empty() || strip_res.iter().any(|re| re.is_match(&normalized)) {
            continue;
        }
        let is_preserved = preserve_res.iter().any(|re| re.is_match(&normalized));
        if is_preserved {
            push(&mut preserved, normalized.clone());
        }
        if !keep_res.is_empty()
            && !keep_res.iter().any(|re| re.is_match(&normalized))
            && !is_preserved
        {
            continue;
        }
        lines.push(normalized);
    }
    if lines.is_empty() && !filter.on_empty.is_empty() {
        lines.push(filter.on_empty.to_string());
    }
    let selected_lines = lines.len();
    lines = select_head_tail(
        lines,
        filter.head_lines,
        filter.tail_lines,
        filter.max_lines,
    );
    Some(FilteredOutput {
        lines,
        preserved,
        selected_lines,
    })
}

fn apply_runtime_filter(filter: &RuntimeFilter, raw: &str) -> Option<FilteredOutput> {
    let text = if filter.strip_ansi {
        strip_ansi_sequences(raw)
    } else {
        raw.to_string()
    };
    let mut lines = Vec::new();
    let mut preserved = Vec::new();
    for line in text.lines() {
        let redacted = redact_public(line);
        let normalized = cap_to_chars(&norm(&redacted), filter.truncate_lines_at);
        if normalized.is_empty()
            || filter
                .strip_lines_matching
                .iter()
                .any(|re| re.is_match(&normalized))
        {
            continue;
        }
        let is_preserved = filter
            .preserve_lines_matching
            .iter()
            .any(|re| re.is_match(&normalized));
        if is_preserved {
            push(&mut preserved, normalized.clone());
        }
        if !filter.keep_lines_matching.is_empty()
            && !filter
                .keep_lines_matching
                .iter()
                .any(|re| re.is_match(&normalized))
            && !is_preserved
        {
            continue;
        }
        lines.push(normalized);
    }
    if lines.is_empty() && !filter.on_empty.is_empty() {
        lines.push(cap_to_chars(
            &norm(&redact_public(&filter.on_empty)),
            filter.truncate_lines_at.min(512),
        ));
    }
    let selected_lines = lines.len();
    lines = select_head_tail(
        lines,
        filter.head_lines,
        filter.tail_lines,
        filter.max_lines,
    );
    Some(FilteredOutput {
        lines,
        preserved,
        selected_lines,
    })
}

impl BuiltInFilter {
    fn is_critical(&self, raw: &str) -> bool {
        self.unsafe_if_matches.iter().any(|pattern| {
            Regex::new(pattern)
                .expect("valid built-in filter unsafe regex")
                .is_match(raw)
        })
    }
}

fn compile_regexes(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|pattern| Regex::new(pattern).expect("valid built-in filter regex"))
        .collect()
}

fn strip_ansi_sequences(text: &str) -> String {
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]")
        .expect("valid ansi regex")
        .replace_all(text, "")
        .to_string()
}

fn cap_to_chars(text: &str, limit: usize) -> String {
    if limit == 0 || text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push('…');
    out.push_str("[tfy: line capped]");
    out
}

fn select_head_tail(mut lines: Vec<String>, head: usize, tail: usize, max: usize) -> Vec<String> {
    if max == 0 || lines.len() <= max {
        return lines;
    }
    let head_count = head.min(max);
    let tail_count = tail.min(max.saturating_sub(head_count + 1));
    let mut selected = lines.drain(..head_count).collect::<Vec<_>>();
    selected.push("...".into());
    if tail_count > 0 {
        let tail_start = lines.len().saturating_sub(tail_count);
        selected.extend(lines.drain(tail_start..));
    }
    selected
}

#[cfg(test)]
pub(crate) fn validate_built_in_filter_fixtures() -> Result<()> {
    for filter in BUILT_IN_FILTERS {
        if filter.fixtures.is_empty() {
            bail!("built-in filter {} has no inline fixtures", filter.family);
        }
        for fixture in filter.fixtures {
            let raw_ref = format!("fixture_{}", fixture.name.replace('-', "_"));
            let risk = if fixture.exit_code != 0 || filter.is_critical(fixture.raw) {
                "critical"
            } else {
                "success"
            };
            let summary = built_in_filter_summary_candidate(
                filter.family,
                filter.family,
                fixture.exit_code,
                fixture.raw,
                &[],
                &raw_ref,
                risk,
            )
            .ok_or_else(|| anyhow::anyhow!("fixture {} produced no summary", fixture.name))?;
            for expected in fixture.expect_contains {
                if !summary.contains(expected) {
                    bail!(
                        "fixture {} for {} missing expected text {:?}: {}",
                        fixture.name,
                        filter.family,
                        expected,
                        summary
                    );
                }
            }
        }
    }
    Ok(())
}

fn rust_strategy_summary_candidate(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> Option<String> {
    match family {
        "git_status" => Some(git_status_summary(cmd, code, raw, evidence, rr, risk)),
        "git_diff" => Some(git_diff_summary(cmd, code, raw, evidence, rr, risk)),
        "git_log" => Some(git_log_summary(cmd, code, raw, evidence, rr, risk)),
        "gh_pr_checks" => Some(checks_summary(
            "gh_pr_checks",
            cmd,
            code,
            raw,
            evidence,
            rr,
            risk,
        )),
        "cargo_test" | "pytest" | "npm_test" | "pnpm_test" | "yarn_test" | "go_test"
        | "maven_test" | "gradle_test" => {
            Some(test_summary(family, cmd, code, raw, evidence, rr, risk))
        }
        "cargo_clippy" | "cargo_build" | "cargo_check" => {
            Some(build_summary(family, cmd, code, raw, evidence, rr, risk))
        }
        "cargo_fmt_check" => Some(cargo_fmt_summary(cmd, code, raw, evidence, rr, risk)),
        "tsc_check" => Some(tsc_summary(cmd, code, raw, evidence, rr, risk)),
        _ => None,
    }
}

fn git_status_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let changed = git_status_changed_count(raw);
    let branch = raw
        .lines()
        .find(|l| l.starts_with("On branch") || l.starts_with("##"))
        .map(redact_public);
    let state = if raw.trim().is_empty() || raw.contains("nothing to commit, working tree clean") {
        "clean"
    } else if risk == "critical" {
        "blocked"
    } else {
        "dirty"
    };
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_status exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- state={state} changed_paths={changed}"));
    if let Some(branch) = branch {
        lines.push(format!("- {branch}"));
    }
    lines.extend(evidence.iter().take(8).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn git_status_changed_count(raw: &str) -> String {
    let porcelain = raw
        .lines()
        .filter(|line| is_porcelain_status_prefix(line.trim_start()))
        .count();
    if porcelain > 0 {
        return porcelain.to_string();
    }
    let long_status = Regex::new(
        r"(?i)^\s*(?:modified|new file|deleted|renamed|copied|both modified|both added|both deleted|unmerged):\s+",
    )
    .expect("valid long git status regex");
    let long_count = raw
        .lines()
        .filter(|line| long_status.is_match(line))
        .count();
    if long_count > 0 {
        long_count.to_string()
    } else if raw.trim().is_empty() || raw.contains("nothing to commit, working tree clean") {
        "0".into()
    } else {
        "unknown".into()
    }
}

fn is_porcelain_status_prefix(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 3
        && bytes[2].is_ascii_whitespace()
        && matches!(
            (bytes[0], bytes[1]),
            (b'M', b' ')
                | (b' ', b'M')
                | (b'A', b' ')
                | (b' ', b'A')
                | (b'D', b' ')
                | (b' ', b'D')
                | (b'R', b' ')
                | (b' ', b'R')
                | (b'C', b' ')
                | (b' ', b'C')
                | (b'?', b'?')
                | (b'U', b'U')
                | (b'A', b'A')
                | (b'D', b'D')
                | (b'A', b'U')
                | (b'U', b'A')
                | (b'D', b'U')
                | (b'U', b'D')
        )
}

fn git_diff_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let patch_files = raw.lines().filter(|l| l.starts_with("diff --git ")).count();
    let stat_files = raw
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            let starts_with_digit = trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
            trimmed.contains('|') && !starts_with_digit
        })
        .count();
    let files = if patch_files > 0 {
        patch_files.to_string()
    } else if stat_files > 0 {
        stat_files.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let hunk_count = raw.lines().filter(|l| l.starts_with("@@ ")).count();
    let hunks = if hunk_count > 0 || raw.trim().is_empty() {
        hunk_count.to_string()
    } else {
        "unknown".into()
    };
    let patch_adds = raw
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++ "))
        .count();
    let patch_dels = raw
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("--- "))
        .count();
    let stat_insertions = stat_summary_count(raw, "insertion");
    let stat_deletions = stat_summary_count(raw, "deletion");
    let adds = if patch_files > 0 {
        patch_adds.to_string()
    } else if let Some(insertions) = stat_insertions {
        insertions.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let dels = if patch_files > 0 {
        patch_dels.to_string()
    } else if let Some(deletions) = stat_deletions {
        deletions.to_string()
    } else if raw.trim().is_empty() {
        "0".into()
    } else {
        "unknown".into()
    };
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_diff exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- files_changed={files} hunks={hunks} added_lines={adds} deleted_lines={dels}"
    ));
    lines.extend(evidence.iter().take(8).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn stat_summary_count(raw: &str, noun: &str) -> Option<usize> {
    let pattern = format!(r"(?i)\b([0-9]+)\s+{}s?\b", regex::escape(noun));
    let re = Regex::new(&pattern).expect("valid git stat count regex");
    let total = raw
        .lines()
        .filter_map(|line| {
            re.captures(line)
                .and_then(|caps| caps[1].parse::<usize>().ok())
        })
        .sum::<usize>();
    if total > 0 {
        Some(total)
    } else {
        None
    }
}

fn git_log_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let commit_count = raw
        .lines()
        .filter(|l| {
            l.starts_with("commit ")
                || Regex::new(r"^[0-9a-f]{7,}\b")
                    .expect("valid log regex")
                    .is_match(l.trim())
        })
        .count();
    let commits = if commit_count > 0 || raw.trim().is_empty() {
        commit_count.to_string()
    } else {
        "unknown".into()
    };
    let first = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| cap(&redact_public(l)))
        .unwrap_or_else(|| "no commits shown".into());
    let mut lines = vec![format!(
        "TFY command summary: {} family=git_log exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- commits_shown={commits}"));
    lines.push(format!("- first={first}"));
    lines.extend(evidence.iter().take(5).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn checks_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let passed = Regex::new(r"(?i)\b(pass(?:ed|ing)?|success)\b")
        .expect("valid checks pass regex")
        .find_iter(raw)
        .count();
    let failed =
        Regex::new(r"(?i)\b(fail(?:ed|ing)?|error|cancel(?:led|ed)|timed? out|action_required)\b")
            .expect("valid checks fail regex")
            .find_iter(raw)
            .count();
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- checks_pass_like={passed} checks_fail_like={failed}"
    ));
    lines.extend(evidence.iter().take(10).map(|e| format!("- {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn test_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let tests = Regex::new(r"(?i)(\d+)\s+(?:tests?|passed)")
        .expect("valid test count regex")
        .captures_iter(raw)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
        .max()
        .or_else(|| {
            Regex::new(r"(?i)Tests run:\s*([0-9]+)")
                .expect("valid maven test count regex")
                .captures_iter(raw)
                .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
                .max()
        })
        .unwrap_or(0);
    let failures = Regex::new(r"(?i)(\d+)\s+(?:failed|failures?)")
        .expect("valid failure count regex")
        .captures_iter(raw)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
        .max()
        .or_else(|| {
            Regex::new(r"(?i)(?:Failures|Errors):\s*([1-9][0-9]*)")
                .expect("valid maven failure count regex")
                .captures_iter(raw)
                .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
                .max()
        })
        .unwrap_or_else(|| if risk == "critical" { 1 } else { 0 });
    let failed_names = collect_lines(
        raw,
        r"(?i)(FAILED|FAILURE!|<<< FAILURE!|failures:|Tests run:|There are test failures|---- .+ stdout|panic|assert|expected|got|left:|right:|Traceback|Error:|AssertionError|org\.|\.java:[0-9]+)",
        10,
    );
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!(
        "- tests_observed={tests} failures_observed={failures}"
    ));
    lines.extend(failed_names.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn build_summary(
    family: &str,
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let warnings = Regex::new(r"(?i)\bwarning[: ]")
        .expect("valid warning regex")
        .find_iter(raw)
        .count();
    let errors = Regex::new(r"(?i)\berror(?:\[|:|s?\b)")
        .expect("valid error regex")
        .find_iter(raw)
        .count();
    let finished = raw
        .lines()
        .find(|l| l.contains("Finished ") || l.contains("Checking ") || l.contains("Compiling "))
        .map(|l| cap(&redact_public(l)))
        .unwrap_or_else(|| "build output summarized".into());
    let mut lines = vec![format!(
        "TFY command summary: {} family={family} exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- warnings={warnings} errors={errors}"));
    lines.push(format!("- {finished}"));
    lines.extend(evidence.iter().take(10).map(|e| format!("- evidence: {e}")));
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn cargo_fmt_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let diff_count = raw
        .lines()
        .filter(|line| line.starts_with("Diff in "))
        .count();
    let diffs = if diff_count > 0 || raw.trim().is_empty() {
        diff_count.to_string()
    } else {
        "unknown".into()
    };
    let diff_lines = collect_lines(raw, r"(?i)(^Diff in |rustfmt|formatted|formatting)", 10);
    let mut lines = vec![format!(
        "TFY command summary: {} family=cargo_fmt_check exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- format_diffs={diffs}"));
    lines.extend(diff_lines.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn tsc_summary(
    cmd: &str,
    code: i32,
    raw: &str,
    evidence: &[String],
    rr: &str,
    risk: &str,
) -> String {
    let diagnostics_count = Regex::new(r"(?m)\berror\s+TS[0-9]+:")
        .expect("valid TypeScript diagnostic count regex")
        .find_iter(raw)
        .count();
    let diagnostics = if diagnostics_count > 0 || raw.trim().is_empty() {
        diagnostics_count.to_string()
    } else {
        "unknown".into()
    };
    let type_errors = collect_lines(raw, r"(?i)(error TS[0-9]+:|Found [0-9]+ errors?)", 10);
    let mut lines = vec![format!(
        "TFY command summary: {} family=tsc_check exit={code} cmd={cmd}",
        risk.to_uppercase()
    )];
    lines.push(format!("- diagnostics={diagnostics}"));
    lines.extend(type_errors.into_iter().map(|x| format!("- {x}")));
    if risk != "success" {
        lines.extend(evidence.iter().take(8).map(|e| format!("- evidence: {e}")));
    }
    lines.push(format!("raw_ref={rr}"));
    lines.push(String::new());
    lines.join("\n")
}

fn collect_lines(raw: &str, pattern: &str, limit: usize) -> Vec<String> {
    let re = Regex::new(pattern).expect("valid collect regex");
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = redact_public(line);
        if re.is_match(&line) {
            push(&mut out, cap(&norm(&line)));
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn redact_public(text: &str) -> String {
    redact_secret_like(&GitGithubToolPolicy::new().redact_for_summary(text))
}

fn redact_secret_like(text: &str) -> String {
    let assignment = Regex::new(
        r"(?i)\b([A-Z0-9_.-]*(?:api[_-]?key|access[_-]?token|auth[_-]?token|token|secret|password))\s*([:=])\s*[A-Za-z0-9._~+/=-]{6,}",
    )
    .expect("valid secret assignment regex");
    let json_field = Regex::new(
        r#"(?i)([\"]?[A-Z0-9_.-]*(?:api[_-]?key|access[_-]?token|auth[_-]?token|token|secret|password)[\"]?\s*:\s*[\"])[^\"\s,}]{6,}([\"]?)"#,
    )
    .expect("valid secret JSON field regex");
    let bearer =
        Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]{8,}").expect("valid bearer secret regex");
    let text = assignment.replace_all(text, "$1$2[REDACTED]");
    let text = json_field.replace_all(&text, "$1[REDACTED]$2");
    bearer.replace_all(&text, "$1 [REDACTED]").to_string()
}

fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let stop = rest.find(['?', '#']).unwrap_or(rest.len());
    let no_query = &rest[..stop];
    let (authority, path) = match no_query.find('/') {
        Some(idx) => (&no_query[..idx], &no_query[idx..]),
        None => (no_query, ""),
    };
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = match host_port.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            format!("{host}:{port}")
        }
        Some((host, _)) => host.to_string(),
        None => host_port.to_string(),
    };
    format!("{scheme}://{host}{path}")
}

fn is_error(raw: &str) -> bool {
    Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL|not\s+successful|unsuccessful|failing|cancel(?:led|ed)|timed?\s+out|action_required|expired|unauthori[sz]ed|forbidden|token\s+expired|authentication|permission\s+denied)")
        .expect("valid error regex")
        .is_match(raw)
}
fn is_success(raw: &str) -> bool {
    let success = Regex::new(
        r"(?i)(\bpassed\b|\bok\b|\bdone\b|\bclean\b|up[ -]?to[ -]?date|nothing to commit|no changes|0 failures?|all checks passed|checks? passed|successful)",
    )
    .expect("valid success regex");
    success.is_match(raw)
}
fn evidence(raw: &str) -> Vec<String> {
    let err = Regex::new(r"(?i)(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL|not\s+successful|unsuccessful|failing|cancel(?:led|ed)|timed?\s+out|action_required|expired|unauthori[sz]ed|forbidden|token\s+expired|authentication|permission\s+denied)").expect("valid error regex");
    let file = Regex::new(r"[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md)(?::\d+)?")
        .expect("valid file reference regex");
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for line in raw.lines() {
        let line = redact_public(line);
        let em = err.find(&line);
        let fm = file.find(&line);
        match (em, fm) {
            (Some(e), Some(f)) => push(&mut errors, excerpt_multi(&line, &[f, e])),
            (Some(e), None) => push(&mut errors, excerpt(&line, e.start(), e.end())),
            (None, Some(f)) => push(&mut files, excerpt(&line, f.start(), f.end())),
            _ => {}
        }
    }
    errors.into_iter().chain(files).take(12).collect()
}
fn push(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && !v.contains(&s) {
        v.push(s)
    }
}
fn excerpt_multi(line: &str, ms: &[regex::Match]) -> String {
    let Some(start) = ms.iter().map(|m| m.start()).min() else {
        return String::new();
    };
    let Some(end) = ms.iter().map(|m| m.end()).max() else {
        return String::new();
    };
    if end - start > 240 {
        ms.iter()
            .map(|m| excerpt(line, m.start(), m.end()))
            .collect::<Vec<_>>()
            .join(" … ")
    } else {
        excerpt(line, start, end)
    }
}
fn excerpt(line: &str, start: usize, end: usize) -> String {
    let limit = 240;
    if line.len() <= limit {
        return norm(line);
    }
    let ctx = ((limit - (end - start).min(limit)) / 2).max(20);
    let s = floor_char_boundary(line, start.saturating_sub(ctx));
    let e = ceil_char_boundary(line, (end + ctx).min(line.len()));
    let mut out = String::new();
    if s > 0 {
        out.push('…')
    }
    out.push_str(&line[s..e]);
    if e < line.len() {
        out.push('…')
    }
    norm(&out)
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

fn norm(s: &str) -> String {
    Regex::new(r"\s+")
        .expect("valid whitespace regex")
        .replace_all(s, " ")
        .trim()
        .to_string()
}
fn cap(s: &str) -> String {
    const LIMIT: usize = 240;
    if s.len() <= LIMIT {
        return s.into();
    }
    let end = s
        .char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx <= LIMIT)
        .last()
        .unwrap_or(0);
    format!("{}…[tfy: line capped]", &s[..end])
}

fn critical(cmd: &str, code: i32, e: &[String], rr: &str) -> String {
    let body = if e.is_empty() {
        "- critical output present; request raw for details".into()
    } else {
        e.iter()
            .map(|x| format!("- {x}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("TFY command summary: CRITICAL exit={code} cmd={cmd}\n{body}\nraw_ref={rr}\n")
}

fn success(cmd: &str, code: i32, raw: &str, rr: &str) -> String {
    let first = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no output");
    let first = redact_public(first);
    format!(
        "TFY command summary: SUCCESS exit={code} cmd={cmd}; {}; raw_ref={rr}\n",
        cap(&first)
    )
}

fn unknown(cmd: &str, code: i32, raw: &str, e: &[String], rr: &str) -> String {
    let lines: Vec<_> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let normalized = norm(l);
            cap(&redact_public(&normalized))
        })
        .collect();
    let mut selected = lines.iter().take(5).cloned().collect::<Vec<_>>();
    if lines.len() > 8 {
        selected.push("...".into());
        selected.extend(
            lines
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        );
    }
    selected.extend(e.iter().take(5).map(|x| format!("evidence: {x}")));
    let body = if selected.is_empty() {
        "no output".into()
    } else {
        selected.join("\n")
    };
    format!("TFY command summary: UNKNOWN exit={code} cmd={cmd}\n{body}\nraw_ref={rr}\n")
}

fn cap_summary_preserving_raw_ref(
    summary: String,
    raw_ref: &str,
    max_summary_bytes: Option<usize>,
) -> String {
    let Some(limit) = max_summary_bytes else {
        return summary;
    };
    if summary.len() <= limit {
        return summary;
    }
    let suffix = format!("\n…\nraw_ref={raw_ref}\n");
    let prefix_limit = limit.saturating_sub(suffix.len());
    let end = floor_char_boundary(&summary, prefix_limit);
    format!("{}{}", &summary[..end], suffix)
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
