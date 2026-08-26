//! Developer task runner, invoked as `cargo run -p xtask -- <task>`.
//! Keeps build/test orchestration out of shell scripts scattered in CI.

use std::process::{Command, ExitCode};

const MUSL_TARGET: &str = "x86_64-unknown-linux-musl";

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();

    let result = match task.as_str() {
        "init-musl" => build_init_musl(),
        "" => {
            print_usage();
            return ExitCode::FAILURE;
        }
        other => {
            eprintln!("xtask: unknown task '{other}'");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- <task>");
    eprintln!("tasks:");
    eprintln!("  init-musl   build layerfs-init statically against {MUSL_TARGET}");
}

fn build_init_musl() -> Result<(), String> {
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "layerfs-init",
            "--release",
            "--target",
            MUSL_TARGET,
        ])
        .status()
        .map_err(|e| format!("failed to spawn cargo: {e}"))?;

    if !status.success() {
        return Err("cargo build failed".to_string());
    }

    Ok(())
}
