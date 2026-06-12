//! CLI tool for generating missing album art.
//!
//! Usage:
//!   sotf-album-art-gen /path/to/album
//!   sotf-album-art-gen /path/to/music --dry-run
//!   sotf-album-art-gen /path/to/music --limit 10 --force

use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use sotf_audio_player::album_art_generation::{
    AlbumArtClientConfig, AlbumArtGenerationClient, CompletionApi, DEFAULT_IMAGE_QUALITY,
    DEFAULT_IMAGE_SIZE, ImageApi, candidate_for_album, write_generated_cover,
};
use sotf_audio_player::library::MusicLibrary;
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CompletionApiArg {
    Auto,
    Chat,
    Responses,
    LmStudioChat,
}

impl From<CompletionApiArg> for CompletionApi {
    fn from(value: CompletionApiArg) -> Self {
        match value {
            CompletionApiArg::Auto => CompletionApi::Auto,
            CompletionApiArg::Chat => CompletionApi::ChatCompletions,
            CompletionApiArg::Responses => CompletionApi::Responses,
            CompletionApiArg::LmStudioChat => CompletionApi::LmStudioChat,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ImageApiArg {
    Auto,
    OpenAi,
    Glm,
}

impl From<ImageApiArg> for ImageApi {
    fn from(value: ImageApiArg) -> Self {
        match value {
            ImageApiArg::Auto => ImageApi::Auto,
            ImageApiArg::OpenAi => ImageApi::OpenAi,
            ImageApiArg::Glm => ImageApi::Glm,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "sotf-album-art-gen")]
#[command(about = "Generate missing album art for local albums")]
#[command(
    long_about = "Scans an album directory or recursive music directory, finds albums without \
                  discovered artwork, asks a text model to enrich album metadata into an image \
                  prompt, asks an image model for square cover art, and writes cover.png into \
                  the album directory."
)]
struct Args {
    /// Album directory or recursive music directory to scan
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// API base URL. OpenAI modes append /v1; lm-studio-chat derives /api/v1/chat.
    #[arg(long, default_value = "https://api.openai.com/v1")]
    api_base_url: String,

    /// Full URL for the text completion endpoint. Overrides --api-base-url for prompt enrichment.
    #[arg(long)]
    completion_url: Option<String>,

    /// Full URL for the image generation endpoint. Overrides --api-base-url for image generation.
    #[arg(long)]
    generation_url: Option<String>,

    /// Request format for prompt enrichment: auto-detects /responses and /api/v1/chat URLs.
    #[arg(long, value_enum, default_value_t = CompletionApiArg::Auto)]
    completion_api: CompletionApiArg,

    /// Image generation request format. Auto selects GLM for glm-image/cogview models.
    #[arg(long, value_enum, default_value_t = ImageApiArg::Auto)]
    image_api: ImageApiArg,

    /// Environment variable containing the text API key. If unset, no Authorization header is sent.
    #[arg(long, default_value = "OPENAI_API_KEY")]
    api_key_env: String,

    /// Environment variable containing the image API key. Official GLM uses ZHIPU_API_KEY.
    #[arg(long, default_value = "ZHIPU_API_KEY")]
    image_api_key_env: String,

    /// Text model used to enrich album metadata into an image prompt
    #[arg(long, default_value = "google/gemma-4-12b-qat")]
    text_model: String,

    /// Image model used to generate square cover art
    #[arg(long, default_value = "glm-image")]
    image_model: String,

    /// Generated image size. GLM's default is 1280x1280.
    #[arg(long, default_value = DEFAULT_IMAGE_SIZE)]
    image_size: String,

    /// Generated image quality for GLM-compatible image endpoints.
    #[arg(long, default_value = DEFAULT_IMAGE_QUALITY)]
    image_quality: String,

    /// Disable the GLM watermark flag.
    #[arg(long)]
    no_image_watermark: bool,

    /// Optional GLM user_id, 6 to 128 characters.
    #[arg(long)]
    image_user_id: Option<String>,

    /// Maximum number of missing-art albums to process
    #[arg(long)]
    limit: Option<usize>,

    /// Print album summaries and enriched prompts without generating or writing images
    #[arg(long)]
    dry_run: bool,

    /// Overwrite cover.png if it already exists and the album has no discovered artwork
    #[arg(long)]
    force: bool,

    /// Print deterministic album metadata summaries before model calls
    #[arg(short, long)]
    verbose: bool,

    /// Print model request URLs, request bodies, response status, and error response bodies.
    #[arg(long)]
    trace_http: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = if std::env::var("RUST_LOG").is_ok() {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    let args = Args::parse();
    run(args).await
}

async fn run(args: Args) -> anyhow::Result<()> {
    if !args.path.is_dir() {
        return Err(anyhow!("{} is not a directory", args.path.display()));
    }

    let api_key = std::env::var(&args.api_key_env).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    let image_api_key = std::env::var(&args.image_api_key_env)
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });

    let client = AlbumArtGenerationClient::new(AlbumArtClientConfig {
        api_base_url: args.api_base_url.clone(),
        completion_url: args.completion_url.clone(),
        generation_url: args.generation_url.clone(),
        completion_api: args.completion_api.into(),
        image_api: args.image_api.into(),
        api_key,
        image_api_key,
        text_model: args.text_model.clone(),
        image_model: args.image_model.clone(),
        image_size: args.image_size.clone(),
        image_quality: args.image_quality.clone(),
        image_watermark: !args.no_image_watermark,
        image_user_id: args.image_user_id.clone(),
        trace_http: args.trace_http || args.verbose,
    })?;

    let mut library = MusicLibrary::new();
    library
        .add_directory(args.path.clone())
        .map_err(|message| anyhow!(message))?;

    println!("Scanning {}", args.path.display());
    library
        .scan_incremental_with_progress(false, None, |tracks, albums| {
            log::debug!("scan progress: {tracks} tracks, {albums} albums");
        })
        .map_err(|error| anyhow!("library scan failed: {error}"))?;

    println!(
        "Found {} albums ({} tracks)",
        library.albums.len(),
        library
            .albums
            .iter()
            .map(|album| album.tracks.len())
            .sum::<usize>()
    );

    let mut processed = 0usize;
    let mut generated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (index, album) in library.albums.iter().enumerate() {
        if args.limit.is_some_and(|limit| processed >= limit) {
            break;
        }

        let candidate = match candidate_for_album(index, album, args.force) {
            Ok(Some(candidate)) => candidate,
            Ok(None) => {
                skipped += 1;
                continue;
            }
            Err(error) => {
                failed += 1;
                eprintln!("Skipping {}: {error}", album.display_name());
                continue;
            }
        };

        processed += 1;
        println!(
            "\n[{}/{}] {}",
            processed,
            args.limit.unwrap_or(library.albums.len()),
            album.display_name()
        );

        if args.verbose {
            println!("{}", candidate.context.short_content());
        }

        let prompt = match client.enrich_prompt(&candidate.context).await {
            Ok(prompt) => prompt,
            Err(error) => {
                failed += 1;
                eprintln!("  Prompt generation failed: {error:#}");
                continue;
            }
        };

        println!("  Prompt: {prompt}");

        if args.dry_run {
            println!("  Dry run: would write {}", candidate.output_path.display());
            continue;
        }

        match client.generate_image(&prompt).await {
            Ok(image_png) => {
                if let Err(error) =
                    write_generated_cover(&candidate.output_path, &image_png, args.force)
                {
                    failed += 1;
                    eprintln!("  Write failed: {error:#}");
                    continue;
                }

                generated += 1;
                println!("  Wrote {}", candidate.output_path.display());
            }
            Err(error) => {
                failed += 1;
                eprintln!("  Image generation failed: {error:#}");
            }
        }
    }

    println!("\nAlbum art generation complete");
    println!("  Processed missing-art albums: {processed}");
    println!("  Generated: {generated}");
    println!("  Skipped with existing art: {skipped}");
    println!("  Failed: {failed}");

    Ok(())
}
