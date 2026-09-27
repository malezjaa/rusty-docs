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
url = "https://docs.example.com"
content_dir = "docs"
content_position = "end"
output_dir = "docs-site"
base_path = "/"

[api]
package = "my-package"
features = ["serde"]
all_features = false
no_default_features = false
```

The `url` is optional. Set `base_path` if the site is served below a path.
Set `content_position` to `"start"` or `"end"` to move the content section.
CLI flags override matching package, feature, content, and output settings.

Add Markdown or MDX files to `docs/`. For example,
`docs/getting-started.mdx` becomes `/docs/content/getting-started`. Add a
`meta.json` file to control the content section's title or page order.

## Limits

- Rustdoc JSON is unstable and requires nightly.
- Some signatures omit generic bounds and `where` clauses. Macro signatures
  use placeholders. Source-code links are not generated.
- The site is a static export and has no server runtime.

## License

MIT. See [LICENSE](LICENSE).
