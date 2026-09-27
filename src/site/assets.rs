use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use crate::config::{ApiOptions, SiteOptions};

pub(super) struct SiteAssets {
    pub logo_url: Option<String>,
    pub logo_dark_url: Option<String>,
    pub favicon_url: Option<String>,
}

pub(super) fn sync(
    site_dir: &Path,
    workspace_root: &Path,
    site: &SiteOptions,
    api: &ApiOptions,
    base_path: &str,
) -> Result<SiteAssets> {
    let destination = site_dir.join("public/_rusty-docs");
    if destination.exists() {
        fs::remove_dir_all(&destination).with_context(|| {
            format!(
                "Could not clear generated asset directory {}",
                destination.display()
            )
        })?;
    }
    fs::create_dir_all(&destination)
        .with_context(|| format!("Could not create {}", destination.display()))?;

    let logo_url = copy_asset(
        site.logo.as_deref(),
        workspace_root,
        &destination,
        "logo",
        base_path,
    )?;
    let logo_dark_url = copy_asset(
        site.logo_dark.as_deref(),
        workspace_root,
        &destination,
        "logo-dark",
        base_path,
    )?;
    let favicon_url = copy_asset(
        site.favicon.as_deref(),
        workspace_root,
        &destination,
        "favicon",
        base_path,
    )?;
    write_theme_stylesheet(site_dir, workspace_root, site, api)?;

    Ok(SiteAssets {
        logo_url,
        logo_dark_url,
        favicon_url,
    })
}

fn copy_asset(
    source: Option<&Path>,
    workspace_root: &Path,
    destination: &Path,
    stem: &str,
    base_path: &str,
) -> Result<Option<String>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let source = super::metadata::resolve_from(workspace_root, source);
    if !source.is_file() {
        bail!("Configured site asset does not exist: {}", source.display());
    }
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .context("Configured site assets must have a file extension")?;
    let filename = format!("{stem}.{extension}");
    fs::copy(&source, destination.join(&filename)).with_context(|| {
        format!(
            "Could not copy site asset {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    let prefix = if base_path == "/" { "" } else { base_path };
    Ok(Some(format!("{prefix}/_rusty-docs/{filename}")))
}

fn write_theme_stylesheet(
    site_dir: &Path,
    workspace_root: &Path,
    site: &SiteOptions,
    api: &ApiOptions,
) -> Result<()> {
    let mut css = format!(
        "@import \"fumadocs-ui/css/{}.css\";\n@import \"fumadocs-ui/css/preset.css\";\n\n",
        site.theme.preset.as_str()
    );
    css.push_str(API_KIND_STYLES);
    if let Some(width) = site.theme.layout_width.as_deref() {
        css.push_str(&format!(
            "\n:root {{\n  --fd-layout-width: {};\n}}\n",
            width.trim()
        ));
    }
    append_kind_overrides(&mut css, &api.kind_colors, false);
    append_kind_overrides(&mut css, &api.kind_colors_dark, true);

    if let Some(custom_css) = site.theme.custom_css.as_deref() {
        let path = super::metadata::resolve_from(workspace_root, custom_css);
        let custom = fs::read_to_string(&path)
            .with_context(|| format!("Could not read custom CSS file {}", path.display()))?;
        css.push_str("\n/* User CSS */\n");
        css.push_str(&custom);
        if !custom.ends_with('\n') {
            css.push('\n');
        }
    }

    let path = site_dir.join("app/rusty-docs-theme.css");
    fs::write(&path, css)
        .with_context(|| format!("Could not write generated theme file {}", path.display()))
}

fn append_kind_overrides(
    css: &mut String,
    colors: &std::collections::BTreeMap<String, String>,
    dark: bool,
) {
    for (kind, color) in colors {
        let prefix = if dark {
            "html.dark [data-kind"
        } else {
            "[data-kind"
        };
        css.push_str(&format!(
            "\n{prefix}=\"{kind}\"] {{\n  --api-kind-color: {};\n}}\n",
            color.trim()
        ));
    }
}

const API_KIND_STYLES: &str = r#"/* Rust API reference colors */
:root {
  --api-kind-crate: #6f42c1;
  --api-kind-module: #0e7490;
  --api-kind-struct: #087f8c;
  --api-kind-enum: #2563eb;
  --api-kind-union: #7c3aed;
  --api-kind-trait: #a16207;
  --api-kind-type-alias: #0284c7;
  --api-kind-primitive: #0e7490;
  --api-kind-function: #15803d;
  --api-kind-constant: #c2410c;
  --api-kind-static: #be123c;
  --api-kind-macro: #9333ea;
  --api-kind-proc-attribute: #be185d;
  --api-kind-reexport: #0f766e;
}

html.dark,
html[data-theme="dark"] {
  --api-kind-crate: #b392f0;
  --api-kind-module: #67e8f9;
  --api-kind-struct: #5eead4;
  --api-kind-enum: #93c5fd;
  --api-kind-union: #c4b5fd;
  --api-kind-trait: #fbbf24;
  --api-kind-type-alias: #7dd3fc;
  --api-kind-primitive: #67e8f9;
  --api-kind-function: #86efac;
  --api-kind-constant: #fdba74;
  --api-kind-static: #fda4af;
  --api-kind-macro: #d8b4fe;
  --api-kind-proc-attribute: #f9a8d4;
  --api-kind-reexport: #5eead4;
}

[data-kind="crate"] { --api-kind-color: var(--api-kind-crate); }
[data-kind="module"] { --api-kind-color: var(--api-kind-module); }
[data-kind="struct"] { --api-kind-color: var(--api-kind-struct); }
[data-kind="enum"] { --api-kind-color: var(--api-kind-enum); }
[data-kind="union"] { --api-kind-color: var(--api-kind-union); }
[data-kind="trait"],
[data-kind="trait_alias"] { --api-kind-color: var(--api-kind-trait); }
[data-kind="type_alias"] { --api-kind-color: var(--api-kind-type-alias); }
[data-kind="primitive"] { --api-kind-color: var(--api-kind-primitive); }
[data-kind="function"] { --api-kind-color: var(--api-kind-function); }
[data-kind="constant"] { --api-kind-color: var(--api-kind-constant); }
[data-kind="static"] { --api-kind-color: var(--api-kind-static); }
[data-kind="macro"],
[data-kind="proc_macro"],
[data-kind="proc_derive"] { --api-kind-color: var(--api-kind-macro); }
[data-kind="proc_attribute"] { --api-kind-color: var(--api-kind-proc-attribute); }
[data-kind="reexport"] { --api-kind-color: var(--api-kind-reexport); }
"#;
