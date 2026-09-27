use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const API_ITEM_KINDS: &[&str] = &[
    "module",
    "struct",
    "enum",
    "union",
    "constant",
    "static",
    "function",
    "type_alias",
    "trait",
    "trait_alias",
    "macro",
    "proc_macro",
    "proc_attribute",
    "proc_derive",
    "reexport",
];

const API_COLOR_KINDS: &[&str] = &[
    "crate",
    "binary",
    "module",
    "struct",
    "enum",
    "union",
    "trait",
    "trait_alias",
    "type_alias",
    "primitive",
    "function",
    "constant",
    "static",
    "macro",
    "proc_macro",
    "proc_attribute",
    "proc_derive",
    "reexport",
    "field",
    "variant",
    "method",
    "associated",
    "implementation",
];

pub const DEFAULT_API_ITEM_ORDER: &[&str] = &[
    "module",
    "struct",
    "enum",
    "union",
    "constant",
    "static",
    "function",
    "type_alias",
    "trait",
    "trait_alias",
    "macro",
    "proc_macro",
    "proc_attribute",
    "proc_derive",
    "reexport",
];

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub site: SiteOptions,
    pub api: ApiOptions,
}

impl Config {
    fn validate(&self) -> Result<()> {
        validate_api_kinds("api.item_order", &self.api.item_order, true)?;
        validate_api_kinds("api.hidden_kinds", &self.api.hidden_kinds, false)?;
        validate_kind_colors("api.kind_colors", &self.api.kind_colors)?;
        validate_kind_colors("api.kind_colors_dark", &self.api.kind_colors_dark)?;
        validate_source_ref(&self.api.source_ref)?;
        validate_css_length(&self.site.theme.layout_width)?;
        if self.site.language.trim().is_empty()
            || self.site.language.chars().any(char::is_whitespace)
        {
            bail!("site.language must be a non-empty language tag without whitespace");
        }
        for (index, link) in self.site.navigation.links.iter().enumerate() {
            validate_link(
                &format!("site.navigation.links[{index}]"),
                &link.text,
                &link.url,
            )?;
        }
        for (index, link) in self.site.home.links.iter().enumerate() {
            validate_link(&format!("site.home.links[{index}]"), &link.text, &link.url)?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SiteOptions {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub repository_url: Option<String>,
    pub logo: Option<PathBuf>,
    pub logo_dark: Option<PathBuf>,
    pub favicon: Option<PathBuf>,
    pub language: String,
    pub direction: Direction,
    pub content_dir: PathBuf,
    pub content_position: ContentPosition,
    pub output_dir: PathBuf,
    pub base_path: String,
    pub theme: ThemeOptions,
    pub navigation: NavigationOptions,
    pub page: PageOptions,
    pub home: HomeOptions,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentPosition {
    Start,
    #[default]
    End,
}

impl ContentPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ThemeOptions {
    pub preset: ThemePreset,
    pub default_mode: ThemeMode,
    pub theme_switch: bool,
    pub theme_switch_mode: ThemeSwitchMode,
    pub layout_width: Option<String>,
    pub custom_css: Option<PathBuf>,
}

impl Default for ThemeOptions {
    fn default() -> Self {
        Self {
            preset: ThemePreset::default(),
            default_mode: ThemeMode::default(),
            theme_switch: true,
            theme_switch_mode: ThemeSwitchMode::default(),
            layout_width: None,
            custom_css: None,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreset {
    #[default]
    Neutral,
    Black,
    Vitepress,
    Dusk,
    Catppuccin,
    Ocean,
    Purple,
    Solar,
    Emerald,
    Ruby,
    Aspen,
}

impl ThemePreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Black => "black",
            Self::Vitepress => "vitepress",
            Self::Dusk => "dusk",
            Self::Catppuccin => "catppuccin",
            Self::Ocean => "ocean",
            Self::Purple => "purple",
            Self::Solar => "solar",
            Self::Emerald => "emerald",
            Self::Ruby => "ruby",
            Self::Aspen => "aspen",
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSwitchMode {
    LightDark,
    #[default]
    LightDarkSystem,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, rename_all(serialize = "camelCase"))]
pub struct NavigationOptions {
    pub navbar: bool,
    pub nav_transparent: NavTransparentMode,
    pub search: bool,
    pub sidebar: bool,
    pub sidebar_collapsible: bool,
    pub sidebar_default_open_level: usize,
    pub prefetch: bool,
    pub tabs: bool,
    pub tab_mode: TabMode,
    pub links: Vec<NavigationLink>,
}

impl Default for NavigationOptions {
    fn default() -> Self {
        Self {
            navbar: true,
            nav_transparent: NavTransparentMode::default(),
            search: true,
            sidebar: true,
            sidebar_collapsible: true,
            sidebar_default_open_level: 0,
            prefetch: true,
            tabs: true,
            tab_mode: TabMode::default(),
            links: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NavTransparentMode {
    Always,
    Top,
    #[default]
    None,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TabMode {
    Top,
    #[default]
    Auto,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all(serialize = "camelCase"))]
pub struct NavigationLink {
    pub text: String,
    pub url: String,
    pub style: NavigationLinkStyle,
    pub active: NavigationLinkActive,
    pub placement: NavigationLinkPlacement,
    pub external: bool,
    pub secondary: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NavigationLinkStyle {
    #[default]
    Link,
    Button,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NavigationLinkActive {
    #[default]
    Url,
    NestedUrl,
    None,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NavigationLinkPlacement {
    Nav,
    Menu,
    #[default]
    All,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, rename_all(serialize = "camelCase"))]
pub struct PageOptions {
    pub toc: bool,
    pub toc_style: TocStyle,
    pub breadcrumbs: bool,
    pub footer: bool,
    pub copy_markdown: bool,
    pub view_markdown: bool,
    pub edit_on_github: bool,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            toc: true,
            toc_style: TocStyle::default(),
            breadcrumbs: true,
            footer: true,
            copy_markdown: true,
            view_markdown: true,
            edit_on_github: false,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TocStyle {
    #[default]
    Normal,
    Clerk,
    Block,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all(serialize = "camelCase"))]
pub struct HomeOptions {
    pub mode: HomeMode,
    pub headline: Option<String>,
    pub links: Vec<HomeLink>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HomeMode {
    #[default]
    Landing,
    Redirect,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HomeLink {
    pub text: String,
    pub url: String,
    pub primary: bool,
}

impl Default for SiteOptions {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            url: None,
            repository_url: None,
            logo: None,
            logo_dark: None,
            favicon: None,
            language: "en".to_owned(),
            direction: Direction::default(),
            content_dir: PathBuf::from("docs"),
            content_position: ContentPosition::default(),
            output_dir: PathBuf::from("docs-site"),
            base_path: "/".to_owned(),
            theme: ThemeOptions::default(),
            navigation: NavigationOptions::default(),
            page: PageOptions::default(),
            home: HomeOptions::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ApiOptions {
    pub package: Option<String>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub default_impls_open: bool,
    pub show_blanket_impls: bool,
    pub show_auto_trait_impls: bool,
    pub item_order: Vec<String>,
    pub hidden_kinds: Vec<String>,
    pub source_links: bool,
    pub source_ref: String,
    pub kind_colors: BTreeMap<String, String>,
    pub kind_colors_dark: BTreeMap<String, String>,
}

impl ApiOptions {
    pub fn ordered_kinds(&self) -> Vec<String> {
        let mut kinds = self.item_order.clone();
        for kind in DEFAULT_API_ITEM_ORDER {
            if !kinds.iter().any(|candidate| candidate == kind) {
                kinds.push((*kind).to_owned());
            }
        }
        kinds
    }

    pub fn hides_kind(&self, kind: &str) -> bool {
        self.hidden_kinds.iter().any(|hidden| hidden == kind)
    }
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            package: None,
            features: Vec::new(),
            all_features: false,
            no_default_features: false,
            default_impls_open: false,
            show_blanket_impls: true,
            show_auto_trait_impls: true,
            item_order: Vec::new(),
            hidden_kinds: Vec::new(),
            source_links: false,
            source_ref: "HEAD".to_owned(),
            kind_colors: BTreeMap::new(),
            kind_colors_dark: BTreeMap::new(),
        }
    }
}

pub fn load(path: Option<PathBuf>, workspace_root: &Path) -> Result<Config> {
    let path = match path {
        Some(path) if path.is_absolute() => path,
        Some(path) => std::env::current_dir()
            .context("Could not read the current directory")?
            .join(path),
        None => workspace_root.join("rusty-docs.toml"),
    };

    if !path.exists() {
        if path
            .file_name()
            .is_some_and(|name| name == "rusty-docs.toml")
        {
            return Ok(Config::default());
        }
        bail!("Configuration file does not exist: {}", path.display());
    }

    let input = fs::read_to_string(&path)
        .with_context(|| format!("Could not read configuration file {}", path.display()))?;
    let config: Config = toml::from_str(&input)
        .with_context(|| format!("Could not parse configuration file {}", path.display()))?;
    config
        .validate()
        .with_context(|| format!("Invalid configuration in {}", path.display()))?;
    Ok(config)
}

fn validate_api_kinds(field: &str, kinds: &[String], reject_duplicates: bool) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for kind in kinds {
        if !API_ITEM_KINDS.contains(&kind.as_str()) {
            bail!("{field} contains unknown API item kind '{kind}'");
        }
        if reject_duplicates && !seen.insert(kind) {
            bail!("{field} contains duplicate API item kind '{kind}'");
        }
    }
    Ok(())
}

fn validate_kind_colors(field: &str, colors: &BTreeMap<String, String>) -> Result<()> {
    for (kind, color) in colors {
        if !API_COLOR_KINDS.contains(&kind.as_str()) {
            bail!("{field} contains unknown API item kind '{kind}'");
        }
        if color.trim().is_empty() || color.chars().any(|character| "{};@".contains(character)) {
            bail!("{field}.{kind} must be one CSS color value");
        }
    }
    Ok(())
}

fn validate_source_ref(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.contains("..")
        || value.contains(['?', '#'])
        || value.chars().any(char::is_whitespace)
    {
        bail!("api.source_ref must be a non-empty Git ref without whitespace, '..', '?' or '#'");
    }
    Ok(())
}

fn validate_css_length(value: &Option<String>) -> Result<()> {
    let Some(value) = value else { return Ok(()) };
    if value.trim().is_empty() || value.chars().any(|character| "{};@".contains(character)) {
        bail!("site.theme.layout_width must be one CSS length value");
    }
    Ok(())
}

fn validate_link(field: &str, text: &str, url: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("{field}.text must not be empty");
    }
    if url.trim().is_empty() {
        bail!("{field}.url must not be empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_site_and_api_customization() {
        let config: Config = toml::from_str(
            r##"
[site]
description = "Reference"
repository_url = "https://github.com/acme/demo"
language = "pl"
direction = "ltr"

[site.theme]
preset = "ocean"
default_mode = "dark"
layout_width = "90rem"

[site.navigation]
sidebar_collapsible = false

[[site.navigation.links]]
text = "Examples"
url = "/examples"
style = "button"

[site.page]
toc_style = "clerk"

[api]
item_order = ["trait", "struct"]
hidden_kinds = ["reexport"]
show_blanket_impls = false
source_links = true

[api.kind_colors]
struct = "#123456"
"##,
        )
        .unwrap();

        config.validate().unwrap();
        assert_eq!(config.site.description.as_deref(), Some("Reference"));
        assert!(matches!(config.site.theme.preset, ThemePreset::Ocean));
        assert!(!config.site.navigation.sidebar_collapsible);
        assert_eq!(config.site.navigation.links.len(), 1);
        assert!(!config.api.show_blanket_impls);
        assert!(config.api.hides_kind("reexport"));
        assert_eq!(config.api.ordered_kinds()[..2], ["trait", "struct"]);
    }

    #[test]
    fn rejects_unknown_and_duplicate_api_kinds() {
        let mut config = Config::default();
        config.api.hidden_kinds.push("class".to_owned());
        assert!(config.validate().is_err());

        config.api.hidden_kinds.clear();
        config.api.item_order = vec!["struct".to_owned(), "struct".to_owned()];
        assert!(config.validate().is_err());
    }
}
