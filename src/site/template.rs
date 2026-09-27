mod sidebar;

use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};

static TEMPLATE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/site");
const MARKER: &str = ".rusty-docs-managed";

pub(super) fn ensure_template(site_dir: &Path) -> Result<()> {
    let marker = site_dir.join(MARKER);
    if marker.is_file() {
        copy_missing_embedded_dir(&TEMPLATE, site_dir)?;
        upgrade_mdx_image_dimensions(site_dir)?;
        sync_api_component_styles(site_dir)?;
        upgrade_api_item_component(site_dir)?;
        upgrade_shiki_dependency(site_dir)?;
        upgrade_api_toc_source(site_dir)?;
        upgrade_api_reference_links(site_dir)?;
        upgrade_api_shared_kinds(site_dir)?;
        sidebar::upgrade_api_sidebar_layout(site_dir)?;
        sidebar::upgrade_content_sidebar_position(site_dir)?;
        return Ok(());
    }

    if site_dir.exists() {
        let mut entries = fs::read_dir(site_dir).with_context(|| {
            format!("Could not inspect output directory {}", site_dir.display())
        })?;
        if entries.next().is_some() {
            bail!(
                "Output directory {} already exists and was not created by rusty-docs. Choose another --out-dir or move the existing files.",
                site_dir.display()
            );
        }
    }

    fs::create_dir_all(site_dir)
        .with_context(|| format!("Could not create output directory {}", site_dir.display()))?;
    copy_embedded_dir(&TEMPLATE, site_dir)?;
    fs::write(&marker, "Managed by rusty-docs.\n")
        .with_context(|| format!("Could not write {}", marker.display()))?;
    Ok(())
}

fn upgrade_mdx_image_dimensions(site_dir: &Path) -> Result<()> {
    let path = site_dir.join("components/mdx.tsx");
    if !path.is_file() {
        return Ok(());
    }

    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read MDX components {}", path.display()))?;
    if source.contains("function normalizeImageDimension(") {
        return Ok(());
    }

    const DEFAULT_IMPORT: &str = "import defaultMdxComponents from 'fumadocs-ui/mdx';";
    const COMPONENTS_EXPORT: &str =
        "export function getMDXComponents(components?: MDXComponents) {";
    const COMPONENTS_SPREAD: &str = "    ...defaultMdxComponents,\n";
    let Some(import_end) = source
        .find(DEFAULT_IMPORT)
        .map(|index| index + DEFAULT_IMPORT.len())
    else {
        return Ok(());
    };
    if !source.contains(COMPONENTS_EXPORT) {
        return Ok(());
    }
    if !source.contains(COMPONENTS_SPREAD) {
        return Ok(());
    }

    source.insert_str(import_end, "\nimport type { ComponentProps } from 'react';");
    let export_start = source
        .find(COMPONENTS_EXPORT)
        .context("MDX components export disappeared during migration")?;
    const IMAGE_HELPERS: &str = r#"const DefaultImage = defaultMdxComponents.img;

function normalizeImageDimension(value: unknown): number | undefined {
  if (value === undefined || value === null || value === '') return undefined;

  const dimension = Number(value);
  if (!Number.isFinite(dimension) || dimension <= 0) return undefined;

  return Math.max(1, Math.round(dimension));
}

function MdxImage({ width, height, ...props }: ComponentProps<'img'>) {
  return (
    <DefaultImage
      {...props}
      width={normalizeImageDimension(width)}
      height={normalizeImageDimension(height)}
    />
  );
}

"#;
    source.insert_str(export_start, IMAGE_HELPERS);
    source = source.replacen(
        COMPONENTS_SPREAD,
        "    ...defaultMdxComponents,\n    img: MdxImage,\n",
        1,
    );

    fs::write(&path, source)
        .with_context(|| format!("Could not update MDX components {}", path.display()))
}

fn upgrade_shiki_dependency(site_dir: &Path) -> Result<()> {
    let package_path = site_dir.join("package.json");
    let lock_path = site_dir.join("package-lock.json");
    if !package_path.is_file() || !lock_path.is_file() {
        return Ok(());
    }

    let mut package: serde_json::Value =
        serde_json::from_slice(&fs::read(&package_path).with_context(|| {
            format!(
                "Could not read site package manifest {}",
                package_path.display()
            )
        })?)
        .with_context(|| {
            format!(
                "Could not parse site package manifest {}",
                package_path.display()
            )
        })?;
    let mut lock: serde_json::Value =
        serde_json::from_slice(&fs::read(&lock_path).with_context(|| {
            format!("Could not read site package lock {}", lock_path.display())
        })?)
        .with_context(|| format!("Could not parse site package lock {}", lock_path.display()))?;
    let Some(version) = lock
        .get("packages")
        .and_then(|packages| packages.get("node_modules/shiki"))
        .and_then(|shiki| shiki.get("version"))
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(());
    };
    let version = format!("^{version}");
    let dependency = serde_json::Value::String(version);
    let package_changed = set_json_dependency(&mut package, "dependencies", "shiki", &dependency);
    let lock_changed = lock
        .get_mut("packages")
        .and_then(|packages| packages.get_mut(""))
        .is_some_and(|root| set_json_dependency(root, "dependencies", "shiki", &dependency));
    if package_changed {
        fs::write(&package_path, serde_json::to_vec_pretty(&package)?).with_context(|| {
            format!(
                "Could not update site package manifest {}",
                package_path.display()
            )
        })?;
    }
    if lock_changed {
        fs::write(&lock_path, serde_json::to_vec_pretty(&lock)?).with_context(|| {
            format!("Could not update site package lock {}", lock_path.display())
        })?;
    }
    Ok(())
}

fn set_json_dependency(
    value: &mut serde_json::Value,
    section: &str,
    name: &str,
    dependency: &serde_json::Value,
) -> bool {
    let Some(section) = value
        .get_mut(section)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return false;
    };
    if section.get(name) == Some(dependency) {
        return false;
    }
    section.insert(name.to_owned(), dependency.clone());
    true
}

fn copy_embedded_dir(source: &Dir<'_>, destination: &Path) -> Result<()> {
    for file in source.files() {
        let path = destination.join(file.path());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create directory {}", parent.display()))?;
        }
        fs::write(&path, file.contents())
            .with_context(|| format!("Could not write template file {}", path.display()))?;
    }
    for directory in source.dirs() {
        let path = destination.join(directory.path());
        fs::create_dir_all(&path)
            .with_context(|| format!("Could not create directory {}", path.display()))?;
        copy_embedded_dir(directory, destination)?;
    }
    Ok(())
}

fn copy_missing_embedded_dir(source: &Dir<'_>, destination: &Path) -> Result<()> {
    for file in source.files() {
        let path = destination.join(file.path());
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create directory {}", parent.display()))?;
        }
        fs::write(&path, file.contents())
            .with_context(|| format!("Could not write template file {}", path.display()))?;
    }
    for directory in source.dirs() {
        let path = destination.join(directory.path());
        fs::create_dir_all(&path)
            .with_context(|| format!("Could not create directory {}", path.display()))?;
        copy_missing_embedded_dir(directory, destination)?;
    }
    Ok(())
}

fn sync_api_component_styles(site_dir: &Path) -> Result<()> {
    const START: &str = "/* Rust API reference component styles */";
    const END: &str = "/* End Rust API reference component styles */";

    let path = site_dir.join("app/global.css");
    if !path.is_file() {
        return Ok(());
    }
    let mut current = fs::read_to_string(&path)
        .with_context(|| format!("Could not read stylesheet {}", path.display()))?;

    let template = TEMPLATE
        .get_file("app/global.css")
        .context("Embedded site template is missing app/global.css")?;
    let template = std::str::from_utf8(template.contents())
        .context("Embedded site stylesheet is not valid UTF-8")?;
    let start = template
        .find(START)
        .context("Embedded site stylesheet is missing API component styles")?;
    let end = template
        .find(END)
        .context("Embedded site stylesheet is missing the API styles end marker")?
        + END.len();
    let api_styles = &template[start..end];

    if let Some(existing_start) = current.find(START) {
        if let Some(existing_end) = current[existing_start..].find(END) {
            let existing_end = existing_end + existing_start + END.len();
            current.replace_range(existing_start..existing_end, api_styles);
        } else {
            append_api_styles(&mut current, api_styles);
        }
    } else {
        append_api_styles(&mut current, api_styles);
    }
    fs::write(&path, current).with_context(|| format!("Could not update {}", path.display()))
}

fn append_api_styles(stylesheet: &mut String, api_styles: &str) {
    if !stylesheet.ends_with('\n') {
        stylesheet.push('\n');
    }
    stylesheet.push('\n');
    stylesheet.push_str(api_styles);
    stylesheet.push('\n');
}

fn upgrade_api_toc_source(site_dir: &Path) -> Result<()> {
    let path = site_dir.join("lib/source.ts");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read Fumadocs source config {}", path.display()))?;
    if source.contains("const apiTocSchema =") {
        return Ok(());
    }
    let template = TEMPLATE
        .get_file("lib/source.ts")
        .context("Embedded site template is missing lib/source.ts")?;
    let template = std::str::from_utf8(template.contents())
        .context("Embedded Fumadocs source config is not valid UTF-8")?;
    let schema_start = template
        .find("const apiTocSchema =")
        .context("Embedded Fumadocs source config is missing the API TOC schema")?;
    let schema_end = template[schema_start..]
        .find("\n\nconst docs =")
        .map(|offset| schema_start + offset)
        .context("Embedded Fumadocs source config is missing the docs collection")?;
    let api_toc_schema = &template[schema_start..schema_end];

    let schema_import = "import { metaSchema, pageSchema } from 'fumadocs-core/source/schema';";
    if source.contains(schema_import) && !source.contains("from 'zod'") {
        source = source.replace(
            schema_import,
            &format!("{schema_import}\nimport {{ z }} from 'zod';"),
        );
    }
    if !source.contains("from 'zod'") && !source.contains("from \"zod\"") {
        if let Some(insertion) = source.find("const docs = defineDocs(") {
            source.insert_str(insertion, "import { z } from 'zod';\n\n");
        }
    }
    let docs_declaration = "const docs = defineDocs(";
    let Some(docs_start) = source.find(docs_declaration) else {
        return Ok(());
    };
    source.insert_str(docs_start, &format!("{api_toc_schema}\n\n"));
    source = source.replace(
        "schema: pageSchema,",
        "schema: pageSchema.extend({ apiToc: apiTocSchema.default([]) }),",
    );
    fs::write(&path, source)
        .with_context(|| format!("Could not update Fumadocs source config {}", path.display()))
}

fn upgrade_api_item_component(site_dir: &Path) -> Result<()> {
    let path = site_dir.join("components/api-item.tsx");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read API item component {}", path.display()))?;
    let original = source.clone();

    if source.contains("from \"lucide-react\"") {
        if !source.contains("  Terminal,") {
            source = source.replace("  Type,\n", "  Type,\n  Terminal,\n");
        }
        if !source.contains("  Package,") {
            source = source.replace(
                "  Layers,\n",
                "  Layers,\n  Package,\n  Rows3,\n  SquareFunction,\n",
            );
        }
        source = source.replace(
            "  function: { label: \"Function\", icon: Code2 },",
            "  function: { label: \"Function\", icon: SquareFunction },",
        );
        source = source.replace(
            "  field: { label: \"Field\", icon: Circle },",
            "  field: { label: \"Field\", icon: Rows3 },",
        );
        if !source.contains("crate: { label: \"Crate\"") {
            source = source.replace(
                "const itemKinds: Record<string, { label: string; icon: LucideIcon }> = {\n",
                "const itemKinds: Record<string, { label: string; icon: LucideIcon }> = {\n  crate: { label: \"Crate\", icon: Package },\n",
            );
        }
        if !source.contains("binary: { label: \"Binary\"") {
            source = source.replace(
                "  crate: { label: \"Crate\", icon: Package },\n",
                "  crate: { label: \"Crate\", icon: Package },\n  binary: { label: \"Binary\", icon: Terminal },\n",
            );
        }
        if !source.contains("icon: Circle },") {
            source = source.replace("  Circle,\n", "");
        }
        source = source.replace(
            "const { label, icon: Icon } = kindInfo(kind);",
            "const { icon: Icon } = kindInfo(kind);",
        );
        source = source.replace("      title={label}\n", "");

        source = source.replace("  groupStart = false,\n", "");
        source = source.replace("  groupStart?: boolean;\n", "");
        source = source.replace(
            r#"className={`api-item-label${compact ? " api-item-label--compact" : ""}${groupStart ? " api-item-label--group-start" : ""}`}"#,
            r#"className={`api-item-label${compact ? " api-item-label--compact" : ""}`}"#,
        );
        source = source.replace(
            "        groupStart ? \"api-item-label--group-start\" : \"\",\n",
            "",
        );
        source = source.replace(
            "className={`api-item-label${compact ? \" api-item-label--compact\" : \"\"}${groupStart ? \" api-item-label--group-start\" : \"\"}`}\n    >",
            "className={`api-item-label${compact ? \" api-item-label--compact\" : \"\"}${groupStart ? \" api-item-label--group-start\" : \"\"}`}\n      data-kind={kind}\n    >",
        );
        source = source.replace(
            "className={`api-item-label${compact ? \" api-item-label--compact\" : \"\"}`}\n    >",
            "className={`api-item-label${compact ? \" api-item-label--compact\" : \"\"}`}\n      data-kind={kind}\n    >",
        );
        source = source.replace(
            "<Link className=\"api-item-link\" href={href} title={rustPath}>",
            "<Link className=\"api-item-link\" data-kind={kind} href={href} title={rustPath}>",
        );

        if !source.contains("api-member__icon-code") {
            source = source.replace(
                "        <ApiItemMark kind={kind} />\n        <pre className=\"api-member__code\">",
                "        <span className=\"api-member__icon-code\">\n          <ApiItemMark compact kind={kind} />\n        </span>\n        <pre className=\"api-member__code\">",
            );
            source = source.replace(
                "        <ApiItemMark kind={kind} compact />\n        <pre className=\"api-member__code\">",
                "        <span className=\"api-member__icon-code\">\n          <ApiItemMark compact kind={kind} />\n        </span>\n        <pre className=\"api-member__code\">",
            );
        }
    }

    let template = TEMPLATE
        .get_file("components/api-item.tsx")
        .context("Embedded site template is missing components/api-item.tsx")?;
    let template = std::str::from_utf8(template.contents())
        .context("Embedded API item component is not valid UTF-8")?;
    let member_start = template
        .find("export function ApiMember(")
        .context("Embedded API item component is missing ApiMember")?;
    let member_end = rust_signature_offset(&template[member_start..])
        .map(|offset| member_start + offset)
        .context("Embedded API item component is missing RustSignature")?;
    let member_components = &template[member_start..member_end];
    let signature_start = rust_signature_offset(template)
        .context("Embedded API item component is missing RustSignature")?
        + 1;
    let signature_component = &template[signature_start..];
    let kinds_start = template
        .find("const itemKinds:")
        .context("Embedded API item component is missing itemKinds")?;
    let kinds_end = template[kinds_start..]
        .find("\n};")
        .map(|offset| kinds_start + offset + "\n};".len())
        .context("Embedded API item component has an incomplete itemKinds map")?;
    let item_kinds = &template[kinds_start..kinds_end];

    if let Some(current_start) = source.find("const itemKinds:")
        && let Some(current_end) = source[current_start..]
            .find("\n};")
            .map(|offset| current_start + offset + "\n};".len())
    {
        source.replace_range(current_start..current_end, item_kinds);
    }

    if !source.contains("from \"shiki\"") {
        let react_import = "import type { CSSProperties, ReactNode } from \"react\";";
        if source.contains(react_import) {
            source = source.replace(
                react_import,
                &format!("{react_import}\nimport {{ codeToTokens }} from \"shiki\";"),
            );
        }
    }

    let mut api_imports = Vec::new();
    if !source.contains("from \"./api-impl-disclosure\"") {
        api_imports.push("import { ApiImplDisclosure } from \"./api-impl-disclosure\";");
    }
    if !source.contains("from \"./api-member-permalink\"") {
        api_imports.push("import { ApiMemberPermalink } from \"./api-member-permalink\";");
    }
    if !api_imports.is_empty() {
        let react_import = "import type { CSSProperties, ReactNode } from \"react\";";
        if source.contains(react_import) {
            source = source.replace(
                react_import,
                &format!("{react_import}\n{}", api_imports.join("\n")),
            );
        }
    }

    if let Some(current_start) = source.find("export function ApiMember(")
        && let Some(current_end) =
            rust_signature_offset(&source[current_start..]).map(|offset| current_start + offset)
    {
        source.replace_range(current_start..current_end, member_components);
    }
    if let Some(current_start) = rust_signature_offset(&source) {
        source.replace_range(current_start + 1.., signature_component);
    }
    if source.matches("Link2").count() == 1 {
        source = source.replace("  Link2,\n", "");
    }
    if source.matches("ChevronRight").count() == 1 {
        source = source.replace("  ChevronRight,\n", "");
    }

    if source != original {
        fs::write(&path, source)
            .with_context(|| format!("Could not update API item component {}", path.display()))?;
    }
    Ok(())
}

fn rust_signature_offset(source: &str) -> Option<usize> {
    source
        .find("\nasync function RustSignature(")
        .or_else(|| source.find("\nfunction RustSignature("))
}

fn upgrade_api_reference_links(site_dir: &Path) -> Result<()> {
    let path = site_dir.join("app/docs/[[...slug]]/page.tsx");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read API docs page {}", path.display()))?;
    let original = source.clone();

    source = source.replace(
        "function ApiAwareLink(props: ComponentProps<'a'>) {",
        "function ApiAwareLink(props: ComponentProps<'a'> & { 'data-kind'?: string }) {",
    );
    source = source.replace(
        "const kind = getApiItemKindForUrl(props.href);",
        "const kind = getApiItemKindForUrl(props.href) ?? props['data-kind'];",
    );
    const OLD_EXTERNAL_LINK_BYPASS: &str =
        "if (props.target === \"_blank\") return <a {...props} />;";
    const STYLED_EXTERNAL_LINK: &str = r#"if (props.target === "_blank") {
      if (!kind) return <a {...props} />;
      return (
        <a
          {...props}
          className={['api-reference-link', props.className].filter(Boolean).join(' ')}
          data-kind={kind}
        >
          {withApiKindIcon(props.children, kind)}
        </a>
      );
    }"#;
    if source.contains(OLD_EXTERNAL_LINK_BYPASS) {
        source = source.replace(OLD_EXTERNAL_LINK_BYPASS, STYLED_EXTERNAL_LINK);
    } else if !source.contains("withApiKindIcon(props.children, kind)") {
        source = source.replace(
            "    if (!props.href) return <a {...props} />;",
            &format!("    if (!props.href) return <a {{...props}} />;\n    {STYLED_EXTERNAL_LINK}"),
        );
    }
    if !source.contains("props.className?.split(/\\s+/).includes('api-reference-link')") {
        source = source.replacen(
            "{withApiKindIcon(props.children, kind)}",
            "{props.className?.split(/\\s+/).includes('api-reference-link')\n            ? props.children\n            : withApiKindIcon(props.children, kind)}",
            1,
        );
    }
    source = source.replace(
        "import type { ComponentProps } from 'react';",
        "import { cloneElement, isValidElement, type ComponentProps, type ReactNode } from 'react';",
    );
    source = source.replace(
        "          <ApiItemMark kind={kind} />\n          {props.children}",
        "          {withApiKindIcon(props.children, kind)}",
    );
    source = source.replace(
        "          <ApiItemMark compact kind={kind} />\n          {props.children}",
        "          {withApiKindIcon(props.children, kind)}",
    );
    source = source.replace(
        "<Link className=\"api-reference-link\" href={props.href}>",
        "<Link className=\"api-reference-link\" data-kind={kind} href={props.href}>",
    );
    if !source.contains("function withApiKindIcon(") {
        const HELPER: &str = r#"function withApiKindIcon(children: ReactNode, kind: string): ReactNode {
  if (isValidElement<{ children?: ReactNode }>(children) && children.type === 'code') {
    return cloneElement(children, {
      children: (
        <>
          <ApiItemMark compact kind={kind} />
          {children.props.children}
        </>
      ),
    });
  }

  return (
    <>
      <ApiItemMark compact kind={kind} />
      {children}
    </>
  );
}

"#;
        if let Some(insertion) = source.find("export async function generateStaticParams()") {
            source.insert_str(insertion, HELPER);
        }
    }

    if !source.contains("function mergeApiToc(") {
        source = source.replace(
            "import { MarkdownCopyButton } from '@/components/markdown-copy-button';",
            "import { MarkdownCopyButton } from '@/components/markdown-copy-button';\nimport type { TOCItemType } from 'fumadocs-core/toc';",
        );
        source = source.replace(
            "  const itemKind = getApiItemKindForSlugs(page.slugs);",
            "  const itemKind = getApiItemKindForSlugs(page.slugs);\n  const toc = mergeApiToc(page.data.toc, page.data.apiToc);",
        );
        source = source.replace("<DocsPage toc={page.data.toc}", "<DocsPage toc={toc}");

        let template = TEMPLATE
            .get_file("app/docs/[[...slug]]/page.tsx")
            .context("Embedded site template is missing the docs page")?;
        let template = std::str::from_utf8(template.contents())
            .context("Embedded docs page is not valid UTF-8")?;
        let merge_start = template
            .find("function mergeApiToc(")
            .context("Embedded docs page is missing mergeApiToc")?;
        let merge_end = template[merge_start..]
            .find("\nfunction withApiKindIcon(")
            .map(|offset| merge_start + offset)
            .context("Embedded docs page is missing withApiKindIcon")?;
        let merge_helper = &template[merge_start..merge_end];
        if let Some(insertion) = source.find("function withApiKindIcon(") {
            source.insert_str(insertion, merge_helper);
        }
    }

    if source != original {
        fs::write(&path, source)
            .with_context(|| format!("Could not update API docs page {}", path.display()))?;
    }
    Ok(())
}

fn upgrade_api_shared_kinds(site_dir: &Path) -> Result<()> {
    let path = site_dir.join("lib/shared.ts");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read shared site helpers {}", path.display()))?;
    let original = source.clone();

    if !source.contains("const apiCrateRoutes = new Set(") {
        const OLD_MAP_END: &str = r#"const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizePageUrl(item.route),
    item.kind,
  ]),
);
"#;
        const NEW_CRATE_ROUTES: &str = r#"const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizePageUrl(item.route),
    item.kind,
  ]),
);
const apiCrateRoutes = new Set(
  siteConfig.packages.map((pkg) => normalizePageUrl(`${docsRoute}/api/${pkg.slug}`)),
);
"#;
        source = source.replace(OLD_MAP_END, NEW_CRATE_ROUTES);
    }
    const OLD_KIND_LOOKUP: &str = "export function getApiItemKindForUrl(url: string) {\n  return apiItemKinds.get(normalizePageUrl(url));\n}";
    const NEW_KIND_LOOKUP: &str = "export function getApiItemKindForUrl(url: string) {\n  const normalizedUrl = normalizePageUrl(url);\n  return apiItemKinds.get(normalizedUrl)\n    ?? (apiCrateRoutes.has(normalizedUrl) ? 'crate' : undefined)\n    ?? getRustdocItemKind(url);\n}";
    const OLD_CRATE_KIND_LOOKUP: &str = "export function getApiItemKindForUrl(url: string) {\n  const normalizedUrl = normalizePageUrl(url);\n  return apiItemKinds.get(normalizedUrl)\n    ?? (apiCrateRoutes.has(normalizedUrl) ? 'crate' : undefined);\n}";
    source = source.replace(OLD_KIND_LOOKUP, NEW_KIND_LOOKUP);
    source = source.replace(OLD_CRATE_KIND_LOOKUP, NEW_KIND_LOOKUP);

    if !source.contains("function getRustdocItemKind(") {
        let template = TEMPLATE
            .get_file("lib/shared.ts")
            .context("Embedded site template is missing lib/shared.ts")?;
        let template = std::str::from_utf8(template.contents())
            .context("Embedded shared site helpers are not valid UTF-8")?;
        let helper_start = template
            .find("function getRustdocItemKind(")
            .context("Embedded shared site helpers are missing Rustdoc kind inference")?;
        source.push_str("\n\n");
        source.push_str(&template[helper_start..]);
    }

    if source != original {
        fs::write(&path, source)
            .with_context(|| format!("Could not update shared site helpers {}", path.display()))?;
    }
    Ok(())
}
