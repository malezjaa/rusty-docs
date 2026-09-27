use std::path::PathBuf;

use clap::{
    Parser,
    builder::styling::{AnsiColor, Styles},
};

const CLI_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Cyan.on_default())
    .usage(AnsiColor::Cyan.on_default())
    .literal(AnsiColor::Green.on_default())
    .placeholder(AnsiColor::BrightBlack.on_default())
    .error(AnsiColor::Red.on_default());

#[derive(Debug, Parser)]
#[command(
    name = "rusty-docs",
    version,
    about = "Generate a static docs site for a Rust project",
    styles = CLI_STYLES
)]
pub struct Cli {
    /// Path to a Cargo.toml file. Cargo searches parent directories by default.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Select one package in a Cargo workspace.
    #[arg(short, long, value_name = "PACKAGE", conflicts_with = "workspace")]
    pub package: Option<String>,

    /// Generate one site containing every library package in the workspace.
    #[arg(long, conflicts_with = "package")]
    pub workspace: bool,

    /// Use a configuration file other than rusty-docs.toml.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Directory containing custom Markdown and MDX content.
    #[arg(long, value_name = "PATH")]
    pub content_dir: Option<PathBuf>,

    /// Directory for the generated Next.js project.
    #[arg(long, value_name = "PATH")]
    pub out_dir: Option<PathBuf>,

    /// Cargo features to enable. Separate features with commas.
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "FEATURE",
        conflicts_with = "all_features"
    )]
    pub features: Vec<String>,

    /// Enable every feature of the selected package.
    #[arg(long, conflicts_with = "features")]
    pub all_features: bool,

    /// Disable the package's default Cargo features.
    #[arg(long)]
    pub no_default_features: bool,

    /// Rust toolchain used for rustdoc JSON output.
    #[arg(long, default_value = "nightly", value_name = "TOOLCHAIN")]
    pub toolchain: String,

    /// Suppress terminal progress output.
    #[arg(short, long)]
    pub quiet: bool,

    /// Show output from Cargo and npm commands.
    #[arg(short, long)]
    pub verbose: bool,
}
