//! WASM plugin builder
//!
//! Compiles Rust projects to WASM for use as ODK schema plugins.

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use std::fs;
use std::process::Command;

/// Build command: compile a Rust project to WASM
pub fn cmd_build(input: Utf8PathBuf, out: Option<Utf8PathBuf>, release: bool) -> Result<()> {
    // Find Cargo.toml
    let cargo_toml = find_cargo_toml(&input)?;
    let project_dir = cargo_toml
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid Cargo.toml path"))?;

    // Canonicalize project directory for reliable path searching
    let project_dir = Utf8PathBuf::try_from(
        fs::canonicalize(project_dir.as_std_path())
            .with_context(|| format!("Failed to canonicalize {}", project_dir))?,
    )?;

    eprintln!("Building WASM plugin from {}...", project_dir);

    // Check cargo is available
    if which::which("cargo").is_err() {
        bail!(
            "cargo not found. Install Rust with:\n\
             curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        );
    }

    // Ensure wasm32-unknown-unknown target is installed
    ensure_wasm_target()?;

    // Build the project
    let mut args = vec!["build", "--target", "wasm32-unknown-unknown"];
    if release {
        args.push("--release");
    }

    eprintln!("  Running: cargo {}", args.join(" "));

    let output = Command::new("cargo")
        .args(&args)
        .current_dir(&project_dir)
        .output()
        .context("Failed to run cargo build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        bail!("cargo build failed");
    }

    // Find the output .wasm file
    let crate_name = get_crate_name(&cargo_toml)?;
    let profile = if release { "release" } else { "debug" };
    let wasm_filename = format!("{}.wasm", crate_name.replace('-', "_"));

    // Try project target dir first, then workspace target dir
    let wasm_file = find_wasm_output(&project_dir, &wasm_filename, profile)?;

    // Determine output path
    let output_path = match out {
        Some(p) => p,
        None => {
            // Default: ./target/odk/<crate_name>.wasm
            let odk_target = project_dir.join("target").join("odk");
            fs::create_dir_all(&odk_target)?;
            odk_target.join(format!("{}.wasm", crate_name))
        }
    };

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Copy the WASM file to output location
    fs::copy(&wasm_file, &output_path)
        .with_context(|| format!("Failed to copy {} to {}", wasm_file, output_path))?;

    eprintln!("Built: {}", output_path);
    eprintln!(
        "\nNext steps:\n  odk compile {} -e @typespec/openapi3",
        output_path
    );

    Ok(())
}

/// Find Cargo.toml in the given directory or its parents
fn find_cargo_toml(input: &Utf8PathBuf) -> Result<Utf8PathBuf> {
    let path = if input.is_file() {
        // If input is a file, look in its parent directory
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Utf8PathBuf::from("."))
    } else {
        input.clone()
    };

    // Check if Cargo.toml exists in this directory
    let cargo_toml = path.join("Cargo.toml");
    if cargo_toml.exists() {
        return Ok(cargo_toml);
    }

    // Search parent directories
    let mut current = path.as_std_path().to_path_buf();
    while current.pop() {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            return Utf8PathBuf::try_from(cargo_toml).context("Invalid UTF-8 in path");
        }
    }

    bail!(
        "No Cargo.toml found in {} or parent directories.\n\
         Run this command from a Rust project directory, or use 'odk init --lang=rust' to create one.",
        input
    )
}

/// Get the crate name from Cargo.toml
fn get_crate_name(cargo_toml: &Utf8PathBuf) -> Result<String> {
    let content =
        fs::read_to_string(cargo_toml).with_context(|| format!("Failed to read {}", cargo_toml))?;

    // Simple TOML parsing for name field
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name") {
            if let Some(value) = line.split('=').nth(1) {
                let name = value.trim().trim_matches('"').trim_matches('\'');
                return Ok(name.to_string());
            }
        }
    }

    bail!("Could not find 'name' in {}", cargo_toml)
}

/// Find the WASM output file, checking project and workspace target directories
fn find_wasm_output(
    project_dir: &camino::Utf8Path,
    wasm_filename: &str,
    profile: &str,
) -> Result<Utf8PathBuf> {
    // Try project's own target directory first
    let project_target = project_dir
        .join("target")
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(wasm_filename);

    if project_target.exists() {
        return Ok(project_target);
    }

    // Search upward for workspace target directory
    let mut current = project_dir.as_std_path().to_path_buf();
    while current.pop() {
        let workspace_target = current
            .join("target")
            .join("wasm32-unknown-unknown")
            .join(profile)
            .join(wasm_filename);

        if workspace_target.exists() {
            return Utf8PathBuf::try_from(workspace_target).context("Invalid UTF-8 in path");
        }

        // Stop if we find a Cargo.toml with [workspace]
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    // This is the workspace root, check its target
                    let workspace_target = current
                        .join("target")
                        .join("wasm32-unknown-unknown")
                        .join(profile)
                        .join(wasm_filename);

                    if workspace_target.exists() {
                        return Utf8PathBuf::try_from(workspace_target)
                            .context("Invalid UTF-8 in path");
                    }
                    break;
                }
            }
        }
    }

    bail!(
        "WASM file not found. Expected: {}\n\
         Checked project and workspace target directories.",
        wasm_filename
    )
}

/// Ensure the wasm32-unknown-unknown target is installed
fn ensure_wasm_target() -> Result<()> {
    // Check if target is already installed
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("Failed to run rustup")?;

    let installed = String::from_utf8_lossy(&output.stdout);
    if installed.contains("wasm32-unknown-unknown") {
        return Ok(());
    }

    // Install the target
    eprintln!("  Installing wasm32-unknown-unknown target...");
    let output = Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .output()
        .context("Failed to run rustup target add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Failed to install wasm32-unknown-unknown target:\n{}",
            stderr
        );
    }

    Ok(())
}
