use anyhow::{Context, Result, anyhow};
use clap::Parser;
use gpui_design_tools::{DesignTokenFormat, import_design_tokens_from_path};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[path = "import-design-tokens/default.rs"]
mod default;
#[path = "import-design-tokens/generate.rs"]
mod generate;
#[path = "import-design-tokens/get.rs"]
mod get;
#[path = "import-design-tokens/hex.rs"]
mod hex;
#[path = "import-design-tokens/misc.rs"]
mod misc;
#[path = "import-design-tokens/parse.rs"]
mod parse;
#[cfg(test)]
#[path = "import-design-tokens/tests.rs"]
mod tests;
#[path = "import-design-tokens/theme.rs"]
mod theme;
#[path = "import-design-tokens/types.rs"]
mod types;

use default::default_app_tokens_path;
use default::default_toolkit_tokens_path;
use generate::generate_theme_file_group;
use theme::theme_configs;
use types::Args;
use types::ThemeConfig;

fn main() -> Result<()> {
    let args = Args::parse();

    if args.toolkit {
        let format = DesignTokenFormat::parse(&args.format)?;
        let tokens_path = args.input.unwrap_or_else(default_toolkit_tokens_path);
        let imported = import_design_tokens_from_path(&tokens_path, format)
            .with_context(|| format!("import {}", tokens_path.display()))?;
        println!(
            "Imported {} toolkit design token(s) from {} across {} preset(s)",
            imported.token_count,
            tokens_path.display(),
            imported.preset_count
        );
        return Ok(());
    }

    let tokens_path = args.input.unwrap_or_else(default_app_tokens_path);

    let tokens_str = std::fs::read_to_string(&tokens_path)
        .with_context(|| format!("read {}", tokens_path.display()))?;
    let tokens: Value = serde_json::from_str(&tokens_str)
        .with_context(|| format!("parse {}", tokens_path.display()))?;

    let theme_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| anyhow!("parent of crates/sotf-tools"))?
        .join("app-gpui")
        .join("app")
        .join("theme");

    let mut generated = HashMap::new();
    let mut configs_by_file: BTreeMap<&'static str, Vec<ThemeConfig>> = BTreeMap::new();

    for config in theme_configs() {
        configs_by_file
            .entry(config.file_name)
            .or_default()
            .push(config);
    }

    for (file_name, configs) in configs_by_file {
        let content = generate_theme_file_group(&tokens, &configs)
            .with_context(|| format!("generating {file_name}"))?;
        let out_path = theme_dir.join(file_name);
        generated.insert(file_name, out_path.clone());
        std::fs::write(&out_path, content.as_bytes())
            .with_context(|| format!("write {}", out_path.display()))?;
        println!("Wrote {}", out_path.display());
    }

    println!(
        "\nGenerated {} theme files from {}",
        generated.len(),
        tokens_path.display()
    );
    Ok(())
}
