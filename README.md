# rusty-docs

`rusty-docs` builds a static documentation site for Rust libraries and
binaries. It combines rustdoc pages with your own Markdown or MDX content.

## Requirements

- Rust and Cargo
- A nightly toolchain for rustdoc JSON
- Node.js and npm

Install nightly with `rustup toolchain install nightly`.

## Use

Install the CLI and run it from a Cargo project:

```sh
cargo install --path .
rusty-docs
rusty-docs --package my-package
rusty-docs --workspace
```

Use `--package NAME` to select one package or `--workspace` to document every
workspace package with library or binary targets. A selected package
contributes its library and all binary targets. The workspace picker starts
with every eligible package selected.

The site keeps library pages under `/docs/api/` and adds a source reference for
each binary under `/docs/binaries/`. Binary references include private Rust
items. They document the program's source, not its command-line usage. Write
usage guides in your content directory, which defaults to `docs/`.

Use `--manifest-path PATH` to select another Cargo manifest. Use
`--features a,b`, `--all-features`, and `--no-default-features` to control
Cargo features. Use `--out-dir PATH` or `--content-dir PATH` to change the
generated site or content directory.

The generated Next.js project defaults to `docs-site/`; its static output is
in `docs-site/out/`.

`npm ci` runs when generated site dependencies are missing or stale.

## Configuration

Create `rusty-docs.toml` at the workspace root:

```toml
[site]
title = "My project"
description = "API reference and guides"
url = "https://docs.example.com"
repository_url = "https://github.com/example/my-project"
content_dir = "docs"
content_position = "end"
output_dir = "docs-site"
base_path = "/"
logo = "docs-assets/logo.svg"
logo_dark = "docs-assets/logo-dark.svg"
favicon = "docs-assets/favicon.svg"
language = "en"
direction = "ltr"

[site.theme]
preset = "ocean"
default_mode = "system"
theme_switch = true
theme_switch_mode = "light-dark-system"
layout_width = "90rem"
custom_css = "docs-assets/custom.css"

[site.navigation]
navbar = true
nav_transparent = "none"
search = true
sidebar = true
sidebar_collapsible = true
sidebar_default_open_level = 1
prefetch = true
tabs = true
tab_mode = "auto"

[[site.navigation.links]]
text = "Examples"
url = "https://github.com/example/my-project/tree/main/examples"
style = "link"
active = "none"
placement = "all"
external = true

[site.page]
toc = true
toc_style = "clerk"
breadcrumbs = true
footer = true
copy_markdown = true
view_markdown = true
edit_on_github = true

[site.home]
mode = "landing"
headline = "Build with My project"

[[site.home.links]]
text = "GitHub"
url = "https://github.com/example/my-project"
primary = false

[api]
package = "my-package"
features = ["serde"]
all_features = false
no_default_features = false
default_impls_open = false
show_blanket_impls = true
show_auto_trait_impls = true
item_order = ["module", "trait", "struct", "enum", "function"]
hidden_kinds = ["reexport"]
source_links = true
source_ref = "main"

[api.kind_colors]
struct = "#087f8c"
trait = "#a16207"

[api.kind_colors_dark]
struct = "#5eead4"
trait = "#fbbf24"
```

All options are optional. The defaults preserve the standard Fumadocs layout.
The available theme presets are `neutral`, `black`, `vitepress`, `dusk`,
`catppuccin`, `ocean`, `purple`, `solar`, `emerald`, `ruby`, and `aspen`.
Set `home.mode` to `"redirect"` to send the home page to `/docs`.

`item_order` changes API section and sidebar ordering. Kinds omitted from the
list keep their default relative order. `hidden_kinds` accepts `module`,
`struct`, `enum`, `union`, `constant`, `static`, `function`, `type_alias`,
`trait`, `trait_alias`, `macro`, `proc_macro`, `proc_attribute`, `proc_derive`,
and `reexport`. Source links require a GitHub repository URL. They point to
`source_ref`, which defaults to `HEAD`.

Set `base_path` if the site is served below a path. Set `content_position` to
`"start"` or `"end"` to move the content section. CLI flags override matching
package, feature, content, and output settings.

Add Markdown or MDX files to `docs/`. For example,
`docs/getting-started.mdx` becomes `/docs/content/getting-started`. Add a
`meta.json` file to control the content section's title or page order.

## Limits

- Rustdoc JSON is unstable and requires nightly.
- Some signatures omit generic bounds and `where` clauses. Macro signatures
  use placeholders.
- The site is a static export and has no server runtime.

## License

MIT. See [LICENSE](LICENSE).
