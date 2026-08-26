use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
    run_dir: PathBuf,
}

impl ArtifactStore {
    pub fn create(root: &Path, run_id: &str) -> Result<Self, ArtifactError> {
        validate_relative(Path::new(run_id))?;
        fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        let run_dir = root.join(run_id);
        ensure_no_symlink_ancestors(&root, &run_dir)?;
        fs::create_dir(&run_dir)?;
        Ok(Self { root, run_dir })
    }

    pub fn open_existing(root: &Path, run_dir: &Path) -> Result<Self, ArtifactError> {
        let root = root.canonicalize()?;
        let run_dir = run_dir.canonicalize()?;
        if !run_dir.starts_with(&root) {
            return Err(ArtifactError::EscapesRoot(run_dir));
        }
        ensure_no_symlink_ancestors(&root, &run_dir)?;
        Ok(Self { root, run_dir })
    }

    pub fn path(&self, relative: impl AsRef<Path>) -> Result<PathBuf, ArtifactError> {
        let relative = relative.as_ref();
        validate_relative(relative)?;
        let path = self.run_dir.join(relative);
        ensure_no_symlink_ancestors(&self.run_dir, &path)?;
        Ok(path)
    }

    pub fn create_dir(&self, relative: impl AsRef<Path>) -> Result<PathBuf, ArtifactError> {
        let path = self.path(relative)?;
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn validate_relative(path: &Path) -> Result<(), ArtifactError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ArtifactError::UnsafeRelative(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_no_symlink_ancestors(root: &Path, path: &Path) -> Result<(), ArtifactError> {
    if !path.starts_with(root) {
        return Err(ArtifactError::EscapesRoot(path.to_path_buf()));
    }
    let mut current = root.to_path_buf();
    for component in path.strip_prefix(root).unwrap_or(path).components() {
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            return Err(ArtifactError::Symlink(current));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Redactor {
    secret_keys: BTreeSet<String>,
    replacements: Vec<(String, String)>,
}

impl Redactor {
    pub fn new(home: Option<&Path>, secrets: impl IntoIterator<Item = String>) -> Self {
        let secret_keys = [
            "authorization",
            "token",
            "access_token",
            "api_token",
            "password",
            "secret",
            "client_secret",
            "credential",
            "credentials",
            "cookie",
            "set-cookie",
            "run_id",
            "x-sotf-dev-run-id",
            "api_key",
            "media_title",
            "track_title",
            "artist",
            "album",
            "media_path",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let mut replacements = secrets
            .into_iter()
            .filter(|secret| !secret.is_empty())
            .map(|secret| (secret, "<redacted>".to_owned()))
            .collect::<Vec<_>>();
        if let Some(home) = home.and_then(Path::to_str) {
            replacements.push((home.to_owned(), "<home>".to_owned()));
        }
        replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0.len()));
        Self {
            secret_keys,
            replacements,
        }
    }

    pub fn text(&self, text: &str) -> String {
        self.replacements
            .iter()
            .fold(text.to_owned(), |value, (from, to)| value.replace(from, to))
    }

    pub fn json(&self, value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, value)| {
                        let value = if self.secret_keys.contains(&key.to_ascii_lowercase()) {
                            Value::String("<redacted>".into())
                        } else {
                            self.json(value)
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            ),
            Value::Array(values) => {
                Value::Array(values.iter().map(|value| self.json(value)).collect())
            }
            Value::String(value) => Value::String(self.text(value)),
            _ => value.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact path must be a safe relative path: {0:?}")]
    UnsafeRelative(PathBuf),
    #[error("artifact path escapes its root: {0:?}")]
    EscapesRoot(PathBuf),
    #[error("artifact path crosses symlink {0:?}")]
    Symlink(PathBuf),
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn contains_paths_and_rejects_parent_components() {
        let root = tempdir().unwrap();
        let store = ArtifactStore::create(root.path(), "run-0123456789abcdef").unwrap();
        assert!(
            store
                .path("logs/stdout.log")
                .unwrap()
                .starts_with(store.run_dir())
        );
        assert!(store.path("../escape").is_err());
        assert!(store.path("/tmp/escape").is_err());
    }

    #[test]
    fn recursively_redacts_json_and_paths() {
        let redactor = Redactor::new(
            Some(Path::new("/Users/test")),
            ["run-secret-value".to_owned()],
        );
        let value = json!({
            "authorization": "Bearer abc",
            "nested": [{"url": "http://x/?token=run-secret-value"}],
            "path": "/Users/test/Music/private.flac"
        });
        let redacted = redactor.json(&value);
        assert_eq!(redacted["authorization"], "<redacted>");
        assert!(!redacted.to_string().contains("run-secret-value"));
        assert!(!redacted.to_string().contains("/Users/test"));
    }

    #[test]
    fn redacts_nested_ndjson_urls_xml_logs_and_environment_dumps() {
        let redactor = Redactor::new(
            Some(Path::new("/Users/test")),
            ["run-secret-value".to_owned(), "api-secret-value".to_owned()],
        );
        let formats = [
            "{\"event\":\"request\",\"run_id\":\"run-secret-value\"}\n",
            "https://127.0.0.1/?token=api-secret-value",
            "<request authorization=\"api-secret-value\">run-secret-value</request>",
            "ERROR media=/Users/test/Music/private.flac token=api-secret-value",
            "HOME=/Users/test\nSOTF_TOKEN=api-secret-value",
        ];
        for text in formats {
            let redacted = redactor.text(text);
            assert!(!redacted.contains("run-secret-value"));
            assert!(!redacted.contains("api-secret-value"));
            assert!(!redacted.contains("/Users/test"));
        }

        let nested = serde_json::json!({
            "outer": [{
                "credentials": {"username": "listener", "password": "private"},
                "media_title": "Private recording",
                "artist": "Private artist",
            }]
        });
        let redacted = redactor.json(&nested).to_string();
        assert!(!redacted.contains("private"));
        assert!(!redacted.contains("Private"));
        assert!(!redacted.contains("listener"));
    }
}
