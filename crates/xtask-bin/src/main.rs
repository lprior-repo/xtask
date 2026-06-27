use std::process::ExitCode;

use clap::{Parser, Subcommand};
use xtask_core::GateScope;
use xtask_lanes::{CheckLane, ClippyLane, FmtLane, LaneRunner, PanicAssertScanLane, run_gate};

#[derive(Parser)]
#[command(name = "xtask", version, about = "Deterministic Rust quality gate")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run scoped quality lanes.
    Gate {
        #[arg(long, default_value = "edit")]
        scope: String,
        #[arg(long, default_value = "json")]
        emit: String,
    },
    /// Report required tools, versions, and policy health.
    Doctor {
        #[arg(long)]
        scope: Option<String>,
    },
    /// Explain a rule and show accepted repairs.
    Explain { rule_id: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Gate { scope, emit } => run_gate_command(&scope, &emit),
        Command::Doctor { scope } => {
            eprintln!(
                "xtask doctor --scope {}",
                scope.unwrap_or_else(|| "full".to_owned())
            );
            ExitCode::SUCCESS
        }
        Command::Explain { rule_id } => {
            eprintln!("xtask explain {rule_id}");
            ExitCode::SUCCESS
        }
    }
}

fn run_gate_command(scope_str: &str, emit: &str) -> ExitCode {
    let scope = parse_scope(scope_str);
    let runners: Vec<Box<dyn LaneRunner>> = build_runners(scope);

    let report = run_gate(scope, &runners);

    if emit == "json" {
        match xtask_output::to_json(&report) {
            Ok(json) => {
                println!("{json}");
            }
            Err(e) => {
                eprintln!("error serializing report: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    match &report {
        xtask_core::Report::Pass { .. } => {
            eprintln!("xtask: PASS");
            ExitCode::SUCCESS
        }
        xtask_core::Report::Reject {
            code_findings,
            gate_failures,
            ..
        } => {
            eprintln!(
                "xtask: REJECT ({} code findings, {} gate failures)",
                code_findings.len(),
                gate_failures.len()
            );
            ExitCode::from(1)
        }
        xtask_core::Report::PolicyError { .. } => {
            eprintln!("xtask: POLICY_ERROR");
            ExitCode::from(2)
        }
        xtask_core::Report::InputError { .. } => {
            eprintln!("xtask: INPUT_ERROR");
            ExitCode::from(3)
        }
    }
}

fn parse_scope(s: &str) -> GateScope {
    match s {
        "prepush" => GateScope::Prepush,
        "full" => GateScope::Full,
        "release" => GateScope::Release,
        _ => GateScope::Edit,
    }
}

fn build_runners(scope: GateScope) -> Vec<Box<dyn LaneRunner>> {
    // For now, all edit-scope lanes are always registered.
    // Prepush/full lanes will be added as they're implemented.
    let _ = scope;
    vec![
        Box::new(FmtLane),
        Box::new(CheckLane),
        Box::new(ClippyLane),
        Box::new(PanicAssertScanLane),
    ]
}
