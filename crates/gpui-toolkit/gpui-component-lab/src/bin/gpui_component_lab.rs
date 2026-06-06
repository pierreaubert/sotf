use anyhow::{Context, Result};
use clap::Parser;
use gpui_component_lab::{StoryDocument, builtin_story_registry, load_story_documents};
use gpui_design_tools::{
    DesignTokenFormat, ensure_passed, validate_current_design_tokens,
    validate_design_tokens_from_path,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, SystemTime};

#[derive(Parser)]
struct Args {
    /// Directory containing `*.story.json` designer state.
    #[arg(long, default_value = "crates/gpui-toolkit/stories")]
    stories_dir: PathBuf,
    /// Watch story/token state and print reload events.
    #[arg(long)]
    watch: bool,
    /// Token JSON files to reload while watching.
    #[arg(long = "token")]
    tokens: Vec<PathBuf>,
    /// Watch Rust sources and relaunch a supervised child process.
    #[arg(long)]
    supervise_rust: bool,
    /// Root scanned by `--supervise-rust`.
    #[arg(long, default_value = "crates/gpui-toolkit")]
    rust_watch_root: PathBuf,
    /// Child command relaunched by `--supervise-rust`.
    #[arg(long)]
    child_command: Option<String>,
    /// Emit the built-in registry as JSON.
    #[arg(long)]
    json: bool,
    /// Validate design conformance before starting.
    #[arg(long)]
    conformance: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.supervise_rust {
        return supervise_rust_source(&args.rust_watch_root, args.child_command.as_deref());
    }

    if args.conformance {
        let report = validate_current_design_tokens()?;
        ensure_passed(&report)?;
    }

    let registry = builtin_story_registry()?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&registry)?);
    } else {
        println!("gpui-component-lab: {} built-in stories", registry.len());
        for story in registry.stories() {
            println!("- {} ({})", story.id, story.crate_name);
        }
    }

    let docs = load_story_documents(&args.stories_dir)?;
    if !docs.is_empty() {
        println!(
            "Loaded {} designer story document(s) from {}",
            docs.len(),
            args.stories_dir.display()
        );
    }

    if args.watch {
        watch_live_state(args.stories_dir, args.tokens)?;
    }

    Ok(())
}

fn watch_live_state(stories_dir: PathBuf, tokens: Vec<PathBuf>) -> Result<()> {
    println!(
        "Watching {} for *.story.json changes",
        stories_dir.display()
    );
    for token in &tokens {
        println!("Watching {} for design token changes", token.display());
    }
    let mut last_seen = latest_story_or_token_modified(&stories_dir, &tokens)?;
    loop {
        std::thread::sleep(Duration::from_millis(750));
        let next = latest_story_or_token_modified(&stories_dir, &tokens)?;
        if next > last_seen {
            let docs: Vec<StoryDocument> = load_story_documents(&stories_dir)?;
            println!("Reloaded {} story document(s)", docs.len());
            for token in &tokens {
                let report = validate_design_tokens_from_path(
                    token,
                    DesignTokenFormat::StyleDictionaryJson,
                )?;
                println!(
                    "Reloaded {} token(s) from {}",
                    report.token_count,
                    token.display()
                );
                ensure_passed(&report)?;
            }
            last_seen = next;
        }
    }
}

fn latest_story_or_token_modified(dir: &Path, tokens: &[PathBuf]) -> Result<SystemTime> {
    let mut latest = latest_story_modified(dir)?;
    for token in tokens {
        if token.exists() {
            latest = latest.max(token.metadata()?.modified()?);
        }
    }
    Ok(latest)
}

fn latest_story_modified(dir: &Path) -> Result<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    if !dir.exists() {
        return Ok(latest);
    }
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".story.json"))
        {
            latest = latest.max(entry.metadata()?.modified()?);
        }
    }
    Ok(latest)
}

fn supervise_rust_source(root: &Path, child_command: Option<&str>) -> Result<()> {
    let command = child_command.context("--supervise-rust requires --child-command")?;
    println!(
        "Watching {} for Rust source changes; relaunching child command safely",
        root.display()
    );
    let mut child = Some(spawn_child(command)?);
    let mut last_seen = latest_rust_modified(root)?;

    loop {
        std::thread::sleep(Duration::from_millis(1000));
        if let Some(running) = child.as_mut() {
            if running.try_wait()?.is_some() {
                child = None;
            }
        }

        let next = latest_rust_modified(root)?;
        if next > last_seen {
            if let Some(mut running) = child.take() {
                let _ = running.kill();
                let _ = running.wait();
            }
            child = Some(spawn_child(command)?);
            last_seen = next;
        }
    }
}

fn spawn_child(command: &str) -> Result<Child> {
    let mut parts = command.split_whitespace();
    let program = parts.next().context("child command must not be empty")?;
    Command::new(program)
        .args(parts)
        .spawn()
        .with_context(|| format!("spawn child command '{command}'"))
}

fn latest_rust_modified(root: &Path) -> Result<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    if !root.exists() {
        return Ok(latest);
    }
    for entry in std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "target")
            {
                continue;
            }
            latest = latest.max(latest_rust_modified(&path)?);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "rs" || ext == "toml")
        {
            latest = latest.max(entry.metadata()?.modified()?);
        }
    }
    Ok(latest)
}
