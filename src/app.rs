use std::path::Path;

use anyhow::Result;

use crate::{cli::Cli, config, metadata, rustdoc, site, terminal::Terminal};

pub fn run(cli: Cli) -> Result<()> {
    let terminal = Terminal::new(cli.quiet, cli.verbose);
    let metadata_stage = terminal.stage("Reading Cargo workspace");
    let metadata = metadata::load(cli.manifest_path.as_deref())?;
    let package_count = terminal.highlight(&metadata.workspace_members.len().to_string());
    metadata_stage.finish(&format!("Found {package_count} workspace packages"));

    let config = config::load(cli.config.clone(), &metadata.workspace_root)?;
    let selected = metadata::select_packages(
        &metadata,
        cli.package.as_deref(),
        config.api.package.as_deref(),
        cli.workspace,
        cli.manifest_path.as_deref(),
    )?;
    if selected.is_empty() {
        anyhow::bail!("No packages were selected.");
    }

    let features = if cli.features.is_empty() {
        config.api.features.clone()
    } else {
        cli.features.clone()
    };
    let all_features = if !cli.features.is_empty() {
        false
    } else {
        cli.all_features || config.api.all_features
    };
    let no_default_features = cli.no_default_features || config.api.no_default_features;

    let mut targets = Vec::new();
    for package in &selected {
        for (target, kind) in package.documentable_targets() {
            targets.push(rustdoc::extract(
                package,
                rustdoc::ExtractOptions {
                    target_name: &target.name,
                    target_kind: kind,
                    target_directory: &metadata.target_directory,
                    toolchain: &cli.toolchain,
                    features: &features,
                    all_features,
                    no_default_features,
                },
                &terminal,
            )?);
        }
    }

    let generated = site::generate_and_build(
        &config,
        &metadata,
        &targets,
        cli.content_dir.as_deref(),
        cli.out_dir.as_deref(),
        &terminal,
    )?;
    print_result(&terminal, &generated.site_dir, &generated.static_dir);
    Ok(())
}

fn print_result(terminal: &Terminal, site_dir: &Path, static_dir: &Path) {
    terminal.note(&format!(
        "Next.js project: {}",
        terminal.format_log_path(site_dir)
    ));
    println!("Static site: {}", terminal.format_path(static_dir));
}
