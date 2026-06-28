// xtask-bin — CLI entrypoint, no derive macros.

use std::process::ExitCode;

use xtask_core::GateScope;
use xtask_lanes::{CheckLane, ClippyLane, FmtLane, LaneRunner, PanicAssertScanLane, run_gate};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str);
    let rest: &[String] = match args.get(2..) {
        Some(r) => r,
        None => &[],
    };

    match subcommand {
        Some("gate") => cmd_gate(rest),
        Some("doctor") => cmd_doctor(rest),
        Some("explain") => cmd_explain(rest),
        Some("--help" | "-h") => {
            print_usage();
            ExitCode::SUCCESS
        }
        Some(cmd) => {
            eprintln!("xtask: unknown command '{cmd}'");
            print_usage();
            ExitCode::from(3)
        }
        None => {
            print_usage();
            ExitCode::from(3)
        }
    }
}

fn cmd_gate(args: &[String]) -> ExitCode {
    let mut scope = "edit";
    let mut emit = "json";
    let mut i = 0;
    while i < args.len() {
        let current = match args.get(i) {
            Some(s) => s.as_str(),
            None => break,
        };
        match current {
            "--scope" => {
                let next = i.saturating_add(1);
                if let Some(s) = args.get(next) {
                    scope = s;
                    i = next.saturating_add(1);
                } else {
                    eprintln!("xtask: --scope requires a value");
                    return ExitCode::from(3);
                }
            }
            "--emit" => {
                let next = i.saturating_add(1);
                if let Some(s) = args.get(next) {
                    emit = s;
                    i = next.saturating_add(1);
                } else {
                    eprintln!("xtask: --emit requires a value");
                    return ExitCode::from(3);
                }
            }
            "--help" | "-h" => {
                eprintln!("xtask gate [--scope edit|prepush|full|release] [--emit json]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("xtask gate: unknown flag '{other}'");
                return ExitCode::from(3);
            }
        }
    }

    let gate_scope = parse_scope(scope);
    let runners = build_runners(gate_scope);
    let report = run_gate(gate_scope, &runners);

    if emit == "json" {
        match xtask_output::to_json(&report) {
            Ok(json) => {
                println!("{json}");
            }
            Err(e) => {
                eprintln!("xtask: error serializing report: {e}");
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

fn cmd_doctor(args: &[String]) -> ExitCode {
    let scope_idx = args.iter().position(|a| a == "--scope");
    let scope = scope_idx
        .and_then(|i| {
            let next = i.saturating_add(1);
            args.get(next).map(String::as_str)
        })
        .unwrap_or("full");
    eprintln!("xtask doctor --scope {scope}");
    // TODO: implement doctor logic
    ExitCode::SUCCESS
}

fn cmd_explain(args: &[String]) -> ExitCode {
    args.first().map_or_else(
        || {
            eprintln!("xtask explain: missing rule-id");
            ExitCode::from(3)
        },
        |rule_id| {
            eprintln!("xtask explain {rule_id}");
            // TODO: implement explain logic
            ExitCode::SUCCESS
        },
    )
}

fn print_usage() {
    eprintln!("xtask — deterministic Rust quality gate");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  xtask gate [--scope edit|prepush|full|release] [--emit json]");
    eprintln!("  xtask doctor [--scope <scope>]");
    eprintln!("  xtask explain <rule-id>");
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
    // Edit-scope lanes are always registered.
    // TODO: register TestLane, SupplyChainLane, FeatureMatrixLane for Prepush+.
    // TODO: register MutantsLane for Full+.
    // TODO: register ArtifactBuildLane for Release.
    let _ = scope;
    vec![
        Box::new(FmtLane),
        Box::new(CheckLane),
        Box::new(ClippyLane),
        Box::new(PanicAssertScanLane),
    ]
}
