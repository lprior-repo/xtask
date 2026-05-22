use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a rule
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleId(pub String);

/// Rule severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Fatal,
    Error,
    Warn,
    Info,
}

/// Rule execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleStatus {
    Pass,
    Fail,
    Inconclusive,
    Skipped,
    NotApplicable,
}

/// A single rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub category: String,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub contract_mapping: Option<String>,
    pub checker: CheckerType,
    pub fitness_function: FitnessFunction,
}

/// Types of rule checkers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckerType {
    /// AST-grep pattern match
    AstGrep { pattern: String },
    /// Regex pattern match
    Regex { pattern: String },
    /// Cargo/clippy lint
    CargoLint { command: String },
    /// Custom Rust function
    Custom { name: String },
    /// Function length/complexity analysis
    FunctionMetrics { max_lines: usize, max_complexity: usize },
    /// Dependency/import scan
    ImportScan { allowed: Vec<String>, forbidden: Vec<String> },
    /// Layer boundary check
    LayerBoundary { core_paths: Vec<String>, shell_paths: Vec<String> },
    /// TCB witness check
    TcbCheck { witness_types: Vec<String> },
}

/// Fitness function for scoring compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FitnessFunction {
    /// Binary: 1.0 if pass, 0.0 if fail
    Binary,
    /// Ratio: compliance ratio (e.g., 3/5 = 0.6)
    Ratio { numerator_field: String, denominator_field: String },
    /// Inverse penalty: 1.0 - (violation_count * penalty)
    InversePenalty { penalty: f64 },
    /// Threshold: 1.0 if under threshold, 0.0 if over
    Threshold { threshold: f64 },
    /// Custom scoring function
    Custom { name: String },
}

/// Result of running a single rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResult {
    pub rule_id: RuleId,
    pub status: RuleStatus,
    pub severity: Severity,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub function: Option<String>,
    pub message: String,
    pub contract_violation: Option<String>,
    pub repair_guidance: Vec<String>,
    pub forbidden_repairs: Vec<String>,
    pub fitness_score: f64,
    pub evidence: Option<Evidence>,
    pub raw_output: Option<String>,
}

/// Evidence artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub command: String,
    pub output_digest: String,
    pub raw_output_path: Option<String>,
    pub duration_ms: u64,
}

/// Aggregated results for a bead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleReport {
    pub bead_id: String,
    pub timestamp: String,
    pub git_commit: Option<String>,
    pub results: Vec<RuleResult>,
    pub summary: RuleSummary,
    pub fitness: FitnessReport,
}

/// Summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub total_rules: usize,
    pub passed: usize,
    pub failed: usize,
    pub inconclusive: usize,
    pub skipped: usize,
    pub fatal_failures: usize,
    pub error_failures: usize,
    pub warn_count: usize,
}

/// Fitness report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessReport {
    pub overall_score: f64,
    pub category_scores: HashMap<String, f64>,
    pub per_crate_scores: HashMap<String, f64>,
    pub trend: Option<FitnessTrend>,
}

/// Trend over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessTrend {
    pub previous_score: f64,
    pub delta: f64,
    pub direction: TrendDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Regressing,
    Stable,
}

impl RuleReport {
    pub fn has_fatal(&self) -> bool {
        self.summary.fatal_failures > 0
    }
    
    pub fn has_errors(&self) -> bool {
        self.summary.error_failures > 0
    }
    
    pub fn is_acceptable(&self) -> bool {
        !self.has_fatal() && !self.has_errors()
    }
}

impl RuleResult {
    pub fn new_pass(rule_id: RuleId, message: impl Into<String>) -> Self {
        Self {
            rule_id,
            status: RuleStatus::Pass,
            severity: Severity::Info,
            file: None,
            line: None,
            column: None,
            function: None,
            message: message.into(),
            contract_violation: None,
            repair_guidance: vec![],
            forbidden_repairs: vec![],
            fitness_score: 1.0,
            evidence: None,
            raw_output: None,
        }
    }
    
    pub fn new_fail(
        rule_id: RuleId,
        severity: Severity,
        message: impl Into<String>,
        file: impl Into<String>,
        line: usize,
    ) -> Self {
        Self {
            rule_id,
            status: RuleStatus::Fail,
            severity,
            file: Some(file.into()),
            line: Some(line),
            column: None,
            function: None,
            message: message.into(),
            contract_violation: None,
            repair_guidance: vec![],
            forbidden_repairs: vec![],
            fitness_score: 0.0,
            evidence: None,
            raw_output: None,
        }
    }
}
