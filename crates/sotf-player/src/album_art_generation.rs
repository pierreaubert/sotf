use crate::library::Album;
use anyhow::{Context, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::ImageFormat;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_ALBUM_ART_FILENAME: &str = "cover.png";
pub const DEFAULT_IMAGE_SIZE: &str = "1280x1280";
pub const DEFAULT_IMAGE_QUALITY: &str = "hd";
const GLM_IMAGE_GENERATION_URL: &str = "https://open.bigmodel.cn/api/paas/v4/images/generations";
const GLM_IMAGE_GENERATION_PATH: &str = "api/paas/v4/images/generations";
const ALBUM_ART_SYSTEM_PROMPT: &str = include_str!("../prompts/system-prompt.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumArtPromptContext {
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub edition: Option<String>,
    pub genres: Vec<String>,
    pub composers: Vec<String>,
    pub conductors: Vec<String>,
    pub performers: Vec<String>,
    pub ensembles: Vec<String>,
    pub track_titles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumArtCandidate {
    pub album_index: usize,
    pub output_path: PathBuf,
    pub context: AlbumArtPromptContext,
}

#[derive(Debug, Clone)]
pub struct AlbumArtClientConfig {
    pub api_base_url: String,
    pub completion_url: Option<String>,
    pub generation_url: Option<String>,
    pub completion_api: CompletionApi,
    pub image_api: ImageApi,
    pub api_key: Option<String>,
    pub image_api_key: Option<String>,
    pub text_model: String,
    pub image_model: String,
    pub image_size: String,
    pub image_quality: String,
    pub image_watermark: bool,
    pub image_user_id: Option<String>,
    pub trace_http: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionApi {
    Auto,
    ChatCompletions,
    Responses,
    LmStudioChat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageApi {
    Auto,
    OpenAi,
    Glm,
}

#[derive(Debug, Clone)]
pub struct GeneratedAlbumArt {
    pub prompt: String,
    pub image_png: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AlbumArtGenerationClient {
    http: reqwest::Client,
    config: AlbumArtClientConfig,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<ResponsesInputMessage>,
    temperature: f32,
    max_output_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ResponsesInputMessage {
    role: &'static str,
    content: Vec<ResponsesContentPart>,
}

#[derive(Debug, Serialize)]
struct ResponsesContentPart {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}

#[derive(Debug, Serialize)]
struct LmStudioChatRequest {
    model: String,
    system_prompt: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[derive(Debug, Serialize)]
struct ImageGenerationRequest {
    model: String,
    prompt: String,
    n: u32,
    size: String,
    response_format: &'static str,
}

#[derive(Debug, Serialize)]
struct GlmImageGenerationRequest {
    model: String,
    prompt: String,
    size: String,
    quality: String,
    watermark_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationResponse {
    data: Vec<ImageGenerationData>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationData {
    b64_json: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PromptJson {
    prompt: String,
}

impl AlbumArtPromptContext {
    pub fn from_album(album: &Album) -> Self {
        Self {
            title: clean_label(&album.title).unwrap_or_else(|| "Unknown Album".to_string()),
            artist: clean_label(&album.artist()).unwrap_or_else(|| "Unknown Artist".to_string()),
            year: album.year,
            edition: album.edition.as_deref().and_then(clean_label),
            genres: collect_unique(album, |track| track.genre.as_deref(), 4),
            composers: collect_unique(album, |track| track.composer.as_deref(), 4),
            conductors: collect_unique(album, |track| track.conductor.as_deref(), 3),
            performers: collect_unique(album, |track| track.performer.as_deref(), 4),
            ensembles: collect_unique(album, |track| track.ensemble.as_deref(), 3),
            track_titles: collect_track_titles(album, 8),
        }
    }

    pub fn short_content(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Album: {}", self.title));
        lines.push(format!("Artist: {}", self.artist));

        if let Some(year) = self.year {
            lines.push(format!("Year: {year}"));
        }
        if let Some(edition) = &self.edition {
            lines.push(format!("Edition: {edition}"));
        }
        push_joined(&mut lines, "Genre", &self.genres);
        push_joined(&mut lines, "Composer", &self.composers);
        push_joined(&mut lines, "Conductor", &self.conductors);
        push_joined(&mut lines, "Performer", &self.performers);
        push_joined(&mut lines, "Ensemble", &self.ensembles);
        push_joined(&mut lines, "Representative tracks", &self.track_titles);

        lines.join("\n")
    }
}

impl AlbumArtGenerationClient {
    pub fn new(config: AlbumArtClientConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self { http, config })
    }

    pub async fn generate(
        &self,
        context: &AlbumArtPromptContext,
    ) -> anyhow::Result<GeneratedAlbumArt> {
        let prompt = self.enrich_prompt(context).await?;
        let image_png = self.generate_image(&prompt).await?;
        Ok(GeneratedAlbumArt { prompt, image_png })
    }

    pub async fn enrich_prompt(&self, context: &AlbumArtPromptContext) -> anyhow::Result<String> {
        let url = completion_api_endpoint(
            &self.config.api_base_url,
            self.config.completion_url.as_deref(),
            self.config.completion_api,
        )?;
        let resolved_api = resolved_completion_api(self.config.completion_api, &url);
        trace_completion_resolution(
            self.config.completion_api,
            resolved_api,
            &url,
            self.config.trace_http,
        );
        match resolved_api {
            CompletionApi::ChatCompletions | CompletionApi::Auto => {
                let request = build_chat_request(&self.config.text_model, context);
                trace_request("completion(chat)", &url, &request, self.config.trace_http);
                let (status, body) =
                    authenticated(self.http.post(url.clone()), self.config.api_key.as_deref())
                        .json(&request)
                        .send()
                        .await
                        .context("text model request failed")?
                        .pipe_response()
                        .await
                        .context("failed to read text model response")?;
                trace_response(
                    "completion(chat)",
                    status,
                    &body,
                    self.config.trace_http,
                    true,
                );
                ensure_success(status, &body, "text model")?;

                let response = serde_json::from_str::<ChatCompletionResponse>(&body)
                    .context("failed to decode text model response")?;

                let content = response
                    .choices
                    .first()
                    .map(|choice| choice.message.content.as_str())
                    .ok_or_else(|| anyhow!("text model response did not include choices"))?;

                normalize_prompt_content(content)
            }
            CompletionApi::Responses => {
                let request = build_responses_request(&self.config.text_model, context);
                trace_request(
                    "completion(responses)",
                    &url,
                    &request,
                    self.config.trace_http,
                );
                let (status, body) =
                    authenticated(self.http.post(url.clone()), self.config.api_key.as_deref())
                        .json(&request)
                        .send()
                        .await
                        .context("responses model request failed")?
                        .pipe_response()
                        .await
                        .context("failed to read responses model response")?;
                trace_response(
                    "completion(responses)",
                    status,
                    &body,
                    self.config.trace_http,
                    true,
                );
                ensure_success(status, &body, "responses model")?;

                let response = serde_json::from_str::<serde_json::Value>(&body)
                    .context("failed to decode responses model response")?;

                let content = extract_responses_text(&response)?;
                normalize_prompt_content(&content)
            }
            CompletionApi::LmStudioChat => {
                let request = build_lm_studio_chat_request(&self.config.text_model, context);
                trace_request(
                    "completion(lm-studio-chat)",
                    &url,
                    &request,
                    self.config.trace_http,
                );
                let (status, body) =
                    authenticated(self.http.post(url.clone()), self.config.api_key.as_deref())
                        .json(&request)
                        .send()
                        .await
                        .context("LM Studio chat request failed")?
                        .pipe_response()
                        .await
                        .context("failed to read LM Studio chat response")?;
                trace_response(
                    "completion(lm-studio-chat)",
                    status,
                    &body,
                    self.config.trace_http,
                    true,
                );
                ensure_success(status, &body, "LM Studio chat model")?;

                let content = extract_lm_studio_chat_text(&body)?;
                normalize_prompt_content(&content)
            }
        }
    }

    pub async fn generate_image(&self, prompt: &str) -> anyhow::Result<Vec<u8>> {
        let resolved_api = resolved_image_api(
            self.config.image_api,
            &self.config.image_model,
            self.config.generation_url.as_deref(),
        );
        let url = image_api_endpoint(
            &self.config.api_base_url,
            self.config.generation_url.as_deref(),
            resolved_api,
        )?;
        trace_image_resolution(
            self.config.image_api,
            resolved_api,
            &url,
            self.config.trace_http,
        );

        match resolved_api {
            ImageApi::Glm => self.generate_glm_image(prompt, url).await,
            ImageApi::OpenAi | ImageApi::Auto => self.generate_openai_image(prompt, url).await,
        }
    }

    async fn generate_openai_image(&self, prompt: &str, url: Url) -> anyhow::Result<Vec<u8>> {
        let request = ImageGenerationRequest {
            model: self.config.image_model.clone(),
            prompt: prompt.to_string(),
            n: 1,
            size: self.config.image_size.clone(),
            response_format: "b64_json",
        };
        trace_request(
            "image generation(openai)",
            &url,
            &request,
            self.config.trace_http,
        );
        let (status, body) =
            authenticated(self.http.post(url.clone()), self.config.api_key.as_deref())
                .json(&request)
                .send()
                .await
                .context("image model request failed")?
                .pipe_response()
                .await
                .context("failed to read image model response")?;
        trace_response(
            "image generation(openai)",
            status,
            &body,
            self.config.trace_http,
            !status.is_success(),
        );
        ensure_success(status, &body, "image model")?;

        let response = serde_json::from_str::<ImageGenerationResponse>(&body)
            .context("failed to decode image model response")?;

        let image = response
            .data
            .first()
            .ok_or_else(|| anyhow!("image model response did not include image data"))?;

        let bytes = if let Some(encoded) = &image.b64_json {
            BASE64
                .decode(encoded)
                .context("image response contained invalid base64")?
        } else if let Some(url) = &image.url {
            self.http
                .get(url)
                .send()
                .await
                .context("failed to download generated image URL")?
                .error_for_status()
                .context("generated image URL returned an error status")?
                .bytes()
                .await
                .context("failed to read generated image bytes")?
                .to_vec()
        } else {
            bail!("image model response did not include b64_json or url");
        };

        normalize_square_png(&bytes)
    }

    async fn generate_glm_image(&self, prompt: &str, url: Url) -> anyhow::Result<Vec<u8>> {
        let request = build_glm_image_request(
            &self.config.image_model,
            prompt,
            &self.config.image_size,
            &self.config.image_quality,
            self.config.image_watermark,
            self.config.image_user_id.clone(),
        )?;
        trace_request(
            "image generation(glm)",
            &url,
            &request,
            self.config.trace_http,
        );

        if is_official_glm_endpoint(&url) && self.config.image_api_key.is_none() {
            bail!("ZHIPU_API_KEY is required for the official GLM image generation endpoint");
        }

        let (status, body) = authenticated(
            self.http.post(url.clone()),
            self.config.image_api_key.as_deref(),
        )
        .json(&request)
        .send()
        .await
        .context("GLM image model request failed")?
        .pipe_response()
        .await
        .context("failed to read GLM image model response")?;
        trace_response(
            "image generation(glm)",
            status,
            &body,
            self.config.trace_http,
            !status.is_success(),
        );
        ensure_success(status, &body, "GLM image model")?;

        let response = serde_json::from_str::<ImageGenerationResponse>(&body)
            .context("failed to decode GLM image model response")?;
        let image = response
            .data
            .first()
            .ok_or_else(|| anyhow!("GLM image model response did not include image data"))?;
        let Some(url) = image.url.as_deref() else {
            bail!("GLM image model response did not include an image URL");
        };

        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .context("failed to download generated GLM image URL")?
            .error_for_status()
            .context("generated GLM image URL returned an error status")?
            .bytes()
            .await
            .context("failed to read generated GLM image bytes")?
            .to_vec();

        normalize_square_png(&bytes)
    }
}

pub fn album_output_directory(album: &Album) -> Option<PathBuf> {
    album
        .tracks
        .iter()
        .find_map(|track| track.path.parent().map(Path::to_path_buf))
}

pub fn generated_cover_path(album: &Album) -> Option<PathBuf> {
    album_output_directory(album).map(|dir| dir.join(DEFAULT_ALBUM_ART_FILENAME))
}

pub fn candidate_for_album(
    album_index: usize,
    album: &Album,
    force: bool,
) -> anyhow::Result<Option<AlbumArtCandidate>> {
    if album.album_art_path.is_some() {
        return Ok(None);
    }

    let Some(output_path) = generated_cover_path(album) else {
        return Ok(None);
    };

    if output_path.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite it",
            output_path.display()
        );
    }

    Ok(Some(AlbumArtCandidate {
        album_index,
        output_path,
        context: AlbumArtPromptContext::from_album(album),
    }))
}

pub fn write_generated_cover(path: &Path, png_bytes: &[u8], force: bool) -> anyhow::Result<()> {
    if path.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite it",
            path.display()
        );
    }

    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} does not have a parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::write(path, png_bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn build_chat_request(model: &str, context: &AlbumArtPromptContext) -> ChatCompletionRequest {
    let (system, user) = prompt_messages(context);
    ChatCompletionRequest {
        model: model.to_string(),
        temperature: 0.7,
        max_tokens: 220,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system,
            },
            ChatMessage {
                role: "user",
                content: user,
            },
        ],
    }
}

fn build_responses_request(model: &str, context: &AlbumArtPromptContext) -> ResponsesRequest {
    let (system, user) = prompt_messages(context);
    ResponsesRequest {
        model: model.to_string(),
        temperature: 0.7,
        max_output_tokens: 220,
        input: vec![
            ResponsesInputMessage {
                role: "system",
                content: vec![ResponsesContentPart {
                    kind: "text",
                    text: system,
                }],
            },
            ResponsesInputMessage {
                role: "user",
                content: vec![ResponsesContentPart {
                    kind: "text",
                    text: user,
                }],
            },
        ],
    }
}

fn build_lm_studio_chat_request(
    model: &str,
    context: &AlbumArtPromptContext,
) -> LmStudioChatRequest {
    let (system, user) = prompt_messages(context);
    LmStudioChatRequest {
        model: model.to_string(),
        system_prompt: system,
        input: user,
    }
}

fn prompt_messages(context: &AlbumArtPromptContext) -> (String, String) {
    (
        ALBUM_ART_SYSTEM_PROMPT.trim().to_string(),
        format!(
            "Create one text-to-image prompt from this album metadata.\n\n{}\n\nThe image must be square album art, visually specific, tasteful, and usable as cover artwork. Include a compact negative instruction at the end.",
            context.short_content()
        ),
    )
}

fn build_glm_image_request(
    model: &str,
    prompt: &str,
    size: &str,
    quality: &str,
    watermark_enabled: bool,
    user_id: Option<String>,
) -> anyhow::Result<GlmImageGenerationRequest> {
    validate_glm_image_options(model, size, user_id.as_deref())?;

    Ok(GlmImageGenerationRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        size: size.to_string(),
        quality: quality.to_string(),
        watermark_enabled,
        user_id,
    })
}

fn completion_endpoint(api: CompletionApi) -> &'static str {
    match api {
        CompletionApi::Responses => "responses",
        CompletionApi::LmStudioChat => "api/v1/chat",
        CompletionApi::Auto | CompletionApi::ChatCompletions => "chat/completions",
    }
}

fn resolved_completion_api(api: CompletionApi, url: &Url) -> CompletionApi {
    let path = url.path().trim_end_matches('/');
    if path.ends_with("/api/v1/chat") || path.ends_with("/chat") {
        return CompletionApi::LmStudioChat;
    }

    match api {
        CompletionApi::Auto => {
            if path.ends_with("/responses") {
                CompletionApi::Responses
            } else {
                CompletionApi::ChatCompletions
            }
        }
        explicit => explicit,
    }
}

fn completion_api_endpoint(
    base: &str,
    override_url: Option<&str>,
    api: CompletionApi,
) -> anyhow::Result<Url> {
    if matches!(api, CompletionApi::LmStudioChat) && override_url.is_none() {
        return lm_studio_chat_endpoint(base);
    }

    api_endpoint(base, override_url, completion_endpoint(api))
}

fn resolved_image_api(api: ImageApi, image_model: &str, override_url: Option<&str>) -> ImageApi {
    if let ImageApi::Auto = api {
        if is_glm_image_model(image_model) || override_url.is_some_and(is_glm_image_url) {
            ImageApi::Glm
        } else {
            ImageApi::OpenAi
        }
    } else {
        api
    }
}

fn image_api_endpoint(
    base: &str,
    override_url: Option<&str>,
    api: ImageApi,
) -> anyhow::Result<Url> {
    match api {
        ImageApi::Glm => glm_image_endpoint(override_url),
        ImageApi::OpenAi | ImageApi::Auto => api_endpoint(base, override_url, "images/generations"),
    }
}

fn glm_image_endpoint(override_url: Option<&str>) -> anyhow::Result<Url> {
    let Some(url) = override_url else {
        return Url::parse(GLM_IMAGE_GENERATION_URL).context("invalid built-in GLM image endpoint");
    };

    let url = url.trim();
    if url.is_empty() {
        bail!("explicit GLM image endpoint URL cannot be empty");
    }

    let mut url =
        Url::parse(url).with_context(|| format!("invalid GLM image endpoint URL: {url}"))?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path(GLM_IMAGE_GENERATION_PATH);
    }
    Ok(url)
}

fn lm_studio_chat_endpoint(base: &str) -> anyhow::Result<Url> {
    let trimmed = base.trim().trim_end_matches('/');
    let native_base = trimmed
        .strip_suffix("/api/v1")
        .or_else(|| trimmed.strip_suffix("/v1"))
        .unwrap_or(trimmed);

    Url::parse(&format!("{native_base}/api/v1/chat"))
        .with_context(|| format!("invalid LM Studio API base URL: {native_base}"))
}

fn api_endpoint(base: &str, override_url: Option<&str>, endpoint: &str) -> anyhow::Result<Url> {
    if let Some(url) = override_url {
        let url = url.trim();
        if url.is_empty() {
            bail!("explicit API endpoint URL for {endpoint} cannot be empty");
        }
        return Url::parse(url).with_context(|| format!("invalid API endpoint URL: {url}"));
    }

    let trimmed = base.trim().trim_end_matches('/');
    let base = if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    };
    Url::parse(&format!("{base}/{endpoint}"))
        .with_context(|| format!("invalid API base URL: {base}"))
}

fn is_glm_image_model(model: &str) -> bool {
    let model = model.trim();
    model == "glm-image" || model.starts_with("cogview-")
}

fn is_glm_image_url(url: &str) -> bool {
    let Ok(url) = Url::parse(url.trim()) else {
        return false;
    };

    url.domain()
        .is_some_and(|domain| domain.ends_with("bigmodel.cn"))
        || url.path().contains("/api/paas/v4/images/generations")
}

fn is_official_glm_endpoint(url: &Url) -> bool {
    url.domain()
        .is_some_and(|domain| domain.ends_with("bigmodel.cn"))
}

fn validate_glm_image_options(
    model: &str,
    size: &str,
    user_id: Option<&str>,
) -> anyhow::Result<()> {
    let Some((width, height)) = size.split_once('x') else {
        bail!("GLM image size must use WIDTHxHEIGHT, got {size}");
    };
    let width: u32 = width
        .parse()
        .with_context(|| format!("GLM image size has invalid width: {size}"))?;
    let height: u32 = height
        .parse()
        .with_context(|| format!("GLM image size has invalid height: {size}"))?;

    let (multiple, max_pixels) = if model.trim() == "glm-image" {
        (32, 1_u64 << 22)
    } else {
        (16, 1_u64 << 21)
    };

    if width < 512 || height < 512 || width > 2048 || height > 2048 {
        bail!("GLM image size must be between 512 and 2048 pixels per side, got {size}");
    }
    if model.trim() == "glm-image" && (width < 1024 || height < 1024) {
        bail!("glm-image size must be at least 1024 pixels per side, got {size}");
    }
    if !width.is_multiple_of(multiple) || !height.is_multiple_of(multiple) {
        bail!("GLM image size must be a multiple of {multiple}, got {size}");
    }
    if u64::from(width) * u64::from(height) > max_pixels {
        bail!("GLM image size exceeds maximum pixel count for {model}: {size}");
    }

    if let Some(user_id) = user_id {
        let len = user_id.chars().count();
        if !(6..=128).contains(&len) {
            bail!("GLM image user_id must be 6 to 128 characters");
        }
    }

    Ok(())
}

trait ResponseTraceExt {
    async fn pipe_response(self) -> Result<(StatusCode, String), reqwest::Error>;
}

impl ResponseTraceExt for reqwest::Response {
    async fn pipe_response(self) -> Result<(StatusCode, String), reqwest::Error> {
        let status = self.status();
        let body = self.text().await?;
        Ok((status, body))
    }
}

fn authenticated(request: RequestBuilder, api_key: Option<&str>) -> RequestBuilder {
    match api_key {
        Some(api_key) if !api_key.trim().is_empty() => request.bearer_auth(api_key.trim()),
        _ => request,
    }
}

fn ensure_success(status: StatusCode, body: &str, label: &str) -> anyhow::Result<()> {
    if status.is_success() {
        return Ok(());
    }

    bail!(
        "{label} returned HTTP {status}: {}",
        truncate_for_trace(body, 4000)
    )
}

fn trace_request<T: Serialize>(label: &str, url: &Url, request: &T, enabled: bool) {
    if !enabled {
        return;
    }

    eprintln!("[album-art-gen] {label} request: POST {url}");
    match serde_json::to_string_pretty(request) {
        Ok(json) => eprintln!("[album-art-gen] {label} request body:\n{json}"),
        Err(error) => eprintln!("[album-art-gen] {label} request body encode failed: {error}"),
    }
}

fn trace_completion_resolution(
    configured: CompletionApi,
    resolved: CompletionApi,
    url: &Url,
    enabled: bool,
) {
    if !enabled {
        return;
    }

    eprintln!(
        "[album-art-gen] completion endpoint resolved: configured={configured:?}, resolved={resolved:?}, url={url}"
    );
}

fn trace_image_resolution(configured: ImageApi, resolved: ImageApi, url: &Url, enabled: bool) {
    if !enabled {
        return;
    }

    eprintln!(
        "[album-art-gen] image endpoint resolved: configured={configured:?}, resolved={resolved:?}, url={url}"
    );
}

fn trace_response(label: &str, status: StatusCode, body: &str, enabled: bool, include_body: bool) {
    if !enabled {
        return;
    }

    eprintln!(
        "[album-art-gen] {label} response: HTTP {status}, {} bytes",
        body.len()
    );
    if include_body {
        eprintln!(
            "[album-art-gen] {label} response body:\n{}",
            truncate_for_trace(body, 4000)
        );
    }
}

fn truncate_for_trace(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push_str("...[truncated]");
    truncated
}

fn extract_responses_text(response: &serde_json::Value) -> anyhow::Result<String> {
    if let Some(text) = response.get("output_text").and_then(|value| value.as_str()) {
        return non_empty_prompt(text);
    }

    let Some(output) = response.get("output").and_then(|value| value.as_array()) else {
        bail!("responses model response did not include output_text or output");
    };

    for item in output {
        let Some(content) = item.get("content").and_then(|value| value.as_array()) else {
            continue;
        };
        for part in content {
            if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                return non_empty_prompt(text);
            }
            if let Some(text) = part.get("content").and_then(|value| value.as_str()) {
                return non_empty_prompt(text);
            }
        }
    }

    bail!("responses model response did not include text content")
}

fn extract_lm_studio_chat_text(body: &str) -> anyhow::Result<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        bail!("LM Studio chat response was empty");
    }

    let Ok(response) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return non_empty_prompt(trimmed);
    };

    for key in ["output", "response", "text", "content", "message"] {
        if let Some(text) = response.get(key).and_then(|value| value.as_str()) {
            return non_empty_prompt(text);
        }
    }

    if let Some(message_content) = response
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
    {
        return non_empty_prompt(message_content);
    }

    if let Some(choice_content) = response
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
    {
        return non_empty_prompt(choice_content);
    }

    bail!(
        "LM Studio chat response did not include recognizable text content: {}",
        truncate_for_trace(trimmed, 1000)
    )
}

fn normalize_prompt_content(content: &str) -> anyhow::Result<String> {
    let trimmed = strip_code_fence(content.trim());
    if trimmed.starts_with('{') {
        let parsed: PromptJson = serde_json::from_str(trimmed)
            .context("text model returned JSON, but it did not contain a valid prompt")?;
        return non_empty_prompt(&parsed.prompt);
    }

    non_empty_prompt(trimmed)
}

fn non_empty_prompt(prompt: &str) -> anyhow::Result<String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        bail!("text model returned an empty prompt");
    }
    Ok(prompt.to_string())
}

fn strip_code_fence(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("```") else {
        return content;
    };
    let Some(end) = rest.rfind("```") else {
        return content;
    };
    let fenced = &rest[..end];
    if let Some((first_line, body)) = fenced.split_once('\n')
        && first_line.trim().chars().all(|c| c.is_ascii_alphabetic())
    {
        return body.trim();
    }
    fenced.trim()
}

fn normalize_square_png(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let image =
        image::load_from_memory(bytes).context("generated image bytes are not decodable")?;
    if image.width() != image.height() {
        bail!(
            "generated image is not square ({}x{})",
            image.width(),
            image.height()
        );
    }

    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .context("failed to encode generated image as PNG")?;
    Ok(output.into_inner())
}

fn collect_unique<F>(album: &Album, field: F, max: usize) -> Vec<String>
where
    F: Fn(&crate::library::Track) -> Option<&str>,
{
    let values: BTreeSet<String> = album
        .tracks
        .iter()
        .filter_map(field)
        .filter_map(clean_label)
        .collect();
    values.into_iter().take(max).collect()
}

fn collect_track_titles(album: &Album, max: usize) -> Vec<String> {
    let mut tracks = album.tracks.clone();
    tracks.sort_by_key(|track| {
        (
            track.disc_number.unwrap_or(0),
            track.track_number.unwrap_or(u32::MAX),
            track.path.clone(),
        )
    });

    let mut seen = BTreeSet::new();
    let mut titles = Vec::new();
    for title in tracks
        .iter()
        .filter_map(|track| track.title.as_deref())
        .filter_map(clean_label)
    {
        let key = title.to_lowercase();
        if seen.insert(key) {
            titles.push(title);
        }
        if titles.len() == max {
            break;
        }
    }
    titles
}

fn clean_label(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(160).collect())
    }
}

fn push_joined(lines: &mut Vec<String>, label: &str, values: &[String]) {
    if !values.is_empty() {
        lines.push(format!("{label}: {}", values.join(", ")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Track;
    use tempfile::tempdir;

    fn album_with_tracks(tracks: Vec<Track>) -> Album {
        Album {
            title: "night transit".to_string(),
            year: Some(2021),
            tracks,
            ..Default::default()
        }
    }

    fn track(path: PathBuf, number: u32, title: &str) -> Track {
        Track {
            path,
            title: Some(title.to_string()),
            artist: Some("Aster Vale".to_string()),
            album_artist: Some("Aster Vale".to_string()),
            track_number: Some(number),
            genre: Some("Ambient Electronic".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn prompt_context_extracts_metadata_without_paths() {
        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![
            track(dir.path().join("02.flac"), 2, "Late Platform"),
            track(dir.path().join("01.flac"), 1, "Signal Bloom"),
        ]);

        let context = AlbumArtPromptContext::from_album(&album);
        let content = context.short_content();

        assert_eq!(context.artist, "Aster Vale");
        assert_eq!(context.track_titles, vec!["Signal Bloom", "Late Platform"]);
        assert!(content.contains("Album: night transit"));
        assert!(content.contains("Genre: Ambient Electronic"));
        assert!(!content.contains(dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn system_prompt_is_embedded_from_repo_file() {
        assert!(!ALBUM_ART_SYSTEM_PROMPT.trim().is_empty());

        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        let context = AlbumArtPromptContext::from_album(&album);

        let (system, _user) = prompt_messages(&context);

        assert_eq!(system, ALBUM_ART_SYSTEM_PROMPT.trim());
    }

    #[test]
    fn candidate_skips_albums_with_discovered_art() {
        let dir = tempdir().unwrap();
        let mut album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        album.album_art_path = Some(dir.path().join("folder.jpg"));

        let candidate = candidate_for_album(0, &album, false).unwrap();

        assert!(candidate.is_none());
    }

    #[test]
    fn candidate_errors_on_existing_cover_without_force() {
        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        std::fs::write(dir.path().join(DEFAULT_ALBUM_ART_FILENAME), b"old").unwrap();

        let error = candidate_for_album(0, &album, false).unwrap_err();

        assert!(error.to_string().contains("--force"));
    }

    #[test]
    fn candidate_allows_existing_cover_with_force() {
        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        std::fs::write(dir.path().join(DEFAULT_ALBUM_ART_FILENAME), b"old").unwrap();

        let candidate = candidate_for_album(7, &album, true).unwrap().unwrap();

        assert_eq!(candidate.album_index, 7);
        assert_eq!(
            candidate.output_path,
            dir.path().join(DEFAULT_ALBUM_ART_FILENAME)
        );
    }

    #[test]
    fn prompt_content_accepts_plain_text_json_and_fenced_json() {
        assert_eq!(
            normalize_prompt_content("Square abstract cover").unwrap(),
            "Square abstract cover"
        );
        assert_eq!(
            normalize_prompt_content(r#"{"prompt":"Square cover, no text"}"#).unwrap(),
            "Square cover, no text"
        );
        assert_eq!(
            normalize_prompt_content("```json\n{\"prompt\":\"Square cover\"}\n```").unwrap(),
            "Square cover"
        );
    }

    #[test]
    fn responses_request_uses_input_field() {
        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        let context = AlbumArtPromptContext::from_album(&album);

        let request = build_responses_request("local-model", &context);
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["model"], "local-model");
        assert!(json.get("input").is_some());
        assert!(json.get("messages").is_none());
        assert_eq!(json["input"][0]["role"], "system");
        assert_eq!(json["input"][1]["role"], "user");
        assert_eq!(json["input"][0]["content"][0]["type"], "text");
        assert!(json["input"][0]["content"][0]["text"].is_string());
        assert_eq!(json["input"][1]["content"][0]["type"], "text");
        assert!(json["input"][1]["content"][0]["text"].is_string());
    }

    #[test]
    fn lm_studio_chat_request_matches_native_api_shape() {
        let dir = tempdir().unwrap();
        let album = album_with_tracks(vec![track(dir.path().join("01.flac"), 1, "Intro")]);
        let context = AlbumArtPromptContext::from_album(&album);

        let request = build_lm_studio_chat_request("google/gemma-4-12b-qat", &context);
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["model"], "google/gemma-4-12b-qat");
        assert!(json["system_prompt"].is_string());
        assert!(json["input"].is_string());
        assert!(json.get("messages").is_none());
        assert!(json.get("role").is_none());
    }

    #[test]
    fn glm_image_request_matches_reference_api_shape() {
        let request = build_glm_image_request(
            "glm-image",
            "Square record cover, no text",
            "1280x1280",
            "hd",
            true,
            Some("sotf-user".to_string()),
        )
        .unwrap();
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["model"], "glm-image");
        assert_eq!(json["prompt"], "Square record cover, no text");
        assert_eq!(json["size"], "1280x1280");
        assert_eq!(json["quality"], "hd");
        assert_eq!(json["watermark_enabled"], true);
        assert_eq!(json["user_id"], "sotf-user");
        assert!(json.get("n").is_none());
        assert!(json.get("response_format").is_none());
    }

    #[test]
    fn glm_image_request_omits_user_id_when_absent() {
        let request =
            build_glm_image_request("glm-image", "Square cover", "1280x1280", "hd", false, None)
                .unwrap();
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["watermark_enabled"], false);
        assert!(json.get("user_id").is_none());
    }

    #[test]
    fn image_api_auto_detects_glm_models_and_urls() {
        assert_eq!(
            resolved_image_api(ImageApi::Auto, "glm-image", None),
            ImageApi::Glm
        );
        assert_eq!(
            resolved_image_api(ImageApi::Auto, "cogview-4", None),
            ImageApi::Glm
        );
        assert_eq!(
            resolved_image_api(
                ImageApi::Auto,
                "gpt-image-1",
                Some("https://open.bigmodel.cn/api/paas/v4/images/generations"),
            ),
            ImageApi::Glm
        );
        assert_eq!(
            resolved_image_api(ImageApi::Auto, "gpt-image-1", None),
            ImageApi::OpenAi
        );
    }

    #[test]
    fn glm_image_endpoint_defaults_to_bigmodel_and_appends_path_for_base_url() {
        assert_eq!(
            image_api_endpoint("https://example.com", None, ImageApi::Glm)
                .unwrap()
                .as_str(),
            "https://open.bigmodel.cn/api/paas/v4/images/generations"
        );
        assert_eq!(
            image_api_endpoint(
                "https://example.com",
                Some("http://192.168.1.37:9999"),
                ImageApi::Glm
            )
            .unwrap()
            .as_str(),
            "http://192.168.1.37:9999/api/paas/v4/images/generations"
        );
        assert_eq!(
            image_api_endpoint(
                "https://example.com",
                Some("http://192.168.1.37:9999/custom/images"),
                ImageApi::Glm,
            )
            .unwrap()
            .as_str(),
            "http://192.168.1.37:9999/custom/images"
        );
    }

    #[test]
    fn validates_glm_image_size_and_user_id() {
        assert!(validate_glm_image_options("glm-image", "1280x1280", Some("sotf-user")).is_ok());
        assert!(validate_glm_image_options("glm-image", "1000x1000", None).is_err());
        assert!(validate_glm_image_options("glm-image", "2048x2048", None).is_ok());
        assert!(validate_glm_image_options("glm-image", "2048x2049", None).is_err());
        assert!(validate_glm_image_options("cogview-4", "512x512", None).is_ok());
        assert!(validate_glm_image_options("cogview-4", "513x512", None).is_err());
        assert!(validate_glm_image_options("glm-image", "1280x1280", Some("short")).is_err());
    }

    #[test]
    fn extracts_lm_studio_chat_text_from_common_shapes() {
        assert_eq!(
            extract_lm_studio_chat_text(r#"{"output":"Native prompt"}"#).unwrap(),
            "Native prompt"
        );
        assert_eq!(
            extract_lm_studio_chat_text(r#"{"choices":[{"message":{"content":"Choice prompt"}}]}"#)
                .unwrap(),
            "Choice prompt"
        );
        assert_eq!(
            extract_lm_studio_chat_text("Plain prompt").unwrap(),
            "Plain prompt"
        );
    }

    #[test]
    fn extracts_responses_output_text() {
        let response = serde_json::json!({
            "output_text": "Square cover prompt"
        });

        assert_eq!(
            extract_responses_text(&response).unwrap(),
            "Square cover prompt"
        );
    }

    #[test]
    fn extracts_responses_nested_text() {
        let response = serde_json::json!({
            "output": [{
                "content": [{
                    "type": "output_text",
                    "text": "Nested square cover prompt"
                }]
            }]
        });

        assert_eq!(
            extract_responses_text(&response).unwrap(),
            "Nested square cover prompt"
        );
    }

    #[test]
    fn normalizes_square_image_to_png() {
        let image = image::DynamicImage::new_rgb8(2, 2);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Jpeg).unwrap();

        let png = normalize_square_png(&bytes.into_inner()).unwrap();

        assert_eq!(image::guess_format(&png).unwrap(), ImageFormat::Png);
    }

    #[test]
    fn rejects_non_square_image() {
        let image = image::DynamicImage::new_rgb8(2, 3);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();

        let error = normalize_square_png(&bytes.into_inner()).unwrap_err();

        assert!(error.to_string().contains("not square"));
    }

    #[test]
    fn api_endpoint_appends_v1_when_missing() {
        assert_eq!(
            api_endpoint("https://example.com", None, "chat/completions")
                .unwrap()
                .as_str(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            api_endpoint("https://example.com/v1/", None, "images/generations")
                .unwrap()
                .as_str(),
            "https://example.com/v1/images/generations"
        );
    }

    #[test]
    fn api_endpoint_uses_explicit_url_when_present() {
        assert_eq!(
            api_endpoint(
                "https://example.com/v1",
                Some("https://llm.local/custom/completion"),
                "chat/completions"
            )
            .unwrap()
            .as_str(),
            "https://llm.local/custom/completion"
        );
    }

    #[test]
    fn lm_studio_chat_endpoint_uses_native_api_path() {
        assert_eq!(
            completion_api_endpoint("http://localhost:1234", None, CompletionApi::LmStudioChat)
                .unwrap()
                .as_str(),
            "http://localhost:1234/api/v1/chat"
        );
        assert_eq!(
            completion_api_endpoint(
                "http://localhost:1234/v1",
                None,
                CompletionApi::LmStudioChat
            )
            .unwrap()
            .as_str(),
            "http://localhost:1234/api/v1/chat"
        );
        assert_eq!(
            completion_api_endpoint(
                "http://localhost:1234/api/v1",
                None,
                CompletionApi::LmStudioChat
            )
            .unwrap()
            .as_str(),
            "http://localhost:1234/api/v1/chat"
        );
    }

    #[test]
    fn completion_api_auto_detects_responses_endpoint() {
        let responses = Url::parse("https://example.com/v1/responses").unwrap();
        let chat = Url::parse("https://example.com/v1/chat/completions").unwrap();
        let lm_studio_chat = Url::parse("http://localhost:1234/api/v1/chat").unwrap();

        assert_eq!(
            resolved_completion_api(CompletionApi::Auto, &responses),
            CompletionApi::Responses
        );
        assert_eq!(
            resolved_completion_api(CompletionApi::Auto, &chat),
            CompletionApi::ChatCompletions
        );
        assert_eq!(
            resolved_completion_api(CompletionApi::Auto, &lm_studio_chat),
            CompletionApi::LmStudioChat
        );
        assert_eq!(
            resolved_completion_api(CompletionApi::Responses, &lm_studio_chat),
            CompletionApi::LmStudioChat
        );
    }
}
