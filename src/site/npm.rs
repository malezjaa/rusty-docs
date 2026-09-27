use std::{
    fs,
    path::Path,
    process::{Command, Output},
    time::SystemTime,
};

use anyhow::{Context, Result, bail};

use crate::terminal::Terminal;

pub(super) fn check_node_tools() -> Result<()> {
    for (name, suggestion) in [
        (
            "node",
            "Install Node.js from https://nodejs.org/ and rerun rusty-docs.",
        ),
        ("npm", "Install npm with Node.js and rerun rusty-docs."),
    ] {
        let status = Command::new(name).arg("--version").output();
        match status {
            Ok(output) if output.status.success() => {}
            _ => bail!("{name} is required to build the Fumadocs site. {suggestion}"),
        }
    }
    Ok(())
}

pub(super) fn install_dependencies_if_needed(site_dir: &Path, terminal: &Terminal) -> Result<()> {
    let node_modules = site_dir.join("node_modules");
    let package_json = site_dir.join("package.json");
    let lockfile = site_dir.join("package-lock.json");
    if !package_json.is_file() {
        bail!(
            "Generated Next.js project is missing {}",
            package_json.display()
        );
    }
    if !lockfile.is_file() {
        bail!(
            "Generated Next.js project is missing {}",
            lockfile.display()
        );
    }
    let installed_at = fs::metadata(&node_modules)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let package_changed = modified_after(&package_json, installed_at);
    let lock_changed = modified_after(&lockfile, installed_at);
    if node_modules.is_dir() && !package_changed && !lock_changed {
        return Ok(());
    }

    let stage = terminal.stage("Installing site dependencies with npm ci");
    run_npm(site_dir, &["ci", "--no-audit", "--no-fund"], terminal)?;
    stage.finish("Site dependencies installed");
    Ok(())
}

fn modified_after(path: &Path, time: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| modified > time)
}

pub(super) fn run_npm(site_dir: &Path, args: &[&str], terminal: &Terminal) -> Result<()> {
    let mut command = Command::new("npm");
    command.args(args).current_dir(site_dir);
    if terminal.is_color_enabled() {
        if std::env::var_os("FORCE_COLOR").is_none() {
            command.env("FORCE_COLOR", "1");
        }
        if std::env::var_os("npm_config_color").is_none() {
            command.env("npm_config_color", "always");
        }
    }
    let output: Output = command
        .output()
        .with_context(|| format!("Could not run npm {}", args.join(" ")))?;
    let verbose = terminal.is_verbose();
    if verbose || !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.trim().is_empty() {
            print!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprint!("{stderr}");
        }
    }
    if !output.status.success() {
        bail!(
            "npm {} failed with exit status {}",
            args.join(" "),
            output.status
        );
    }
    Ok(())
}
