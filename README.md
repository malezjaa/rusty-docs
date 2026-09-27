# rusty-docs

`rusty-docs` builds one static documentation site from a Cargo package. The
site combines public Rust API pages with Markdown or MDX guides and uses the
Fumadocs static Next.js starter.

## Requirements

- Rust and Cargo
- A nightly Rust toolchain for rustdoc JSON output
- Node.js and npm for the Next.js build

With rustup, install the nightly toolchain with:

```sh
rustup toolchain install nightly
```

The CLI does not install Rust toolchains. It runs `npm ci` when the generated
site's dependencies are missing or stale.

## Install and run

Install the CLI from this repository:

```sh
cargo install --path .
```

Then run it from a Rust project:

```sh
rusty-docs
rusty-docs --package my-library
rusty-docs --workspace
```

The generated Next.js project lives in `docs-site/`. The static website is in
`docs-site/out/`.

To inspect the build, run `npm start` from the generated project directory.
The preview server serves the existing `out/` files at the configured base
path. It redirects the root URL when `base_path` is not `/`.

Use `--manifest-path PATH` to point to another Cargo manifest. Use
`--features feature-a,feature-b`, `--all-features`, and
`--no-default-features` to choose Cargo features. Use `--out-dir PATH` or
`--content-dir PATH` to change the generated app or the source guide directory.

## Configuration

Create `rusty-docs.toml` at the Cargo workspace root:

```toml
[site]
title = "My library"
url = "https://docs.example.com"
content_dir = "docs"
output_dir = "docs-site"
base_path = "/"

[api]
package = "my-library"
features = ["serde"]
all_features = false
no_default_features = false
```

The `url` field is optional. Set it to the site's canonical origin. Set
`base_path` when the host serves the site below a URL path.

The CLI flags override matching package, feature, content, and output settings.
Source Markdown and MDX files stay in `content_dir`. The CLI copies them into
the generated site's Guides section on each build. It owns the generated
app's `content/docs/` directory and preserves other app files. The generated
site keeps the default Fumadocs styles.

## Current limits

- rustdoc JSON is unstable and requires nightly.
- The CLI documents library targets. It does not generate pages for binary
  targets.
- The CLI documents public modules, structs, enums, traits, type aliases,
  constants, statics, unions, functions, macros, and named re-exports. It does
  not document private items or glob re-exports.
- Some signatures omit generic bounds and `where` clauses. Macro signatures
  use placeholders. External links and source-code links are not generated.
- Static export has no server runtime. Search uses the Fumadocs static client.

## License

Licensed under the MIT license. See [LICENSE](LICENSE).
