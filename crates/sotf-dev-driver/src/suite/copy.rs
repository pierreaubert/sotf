use super::types::RoomEqConfig;
use anyhow::{Context, Result, bail};
use std::fs::{self};
use std::path::{Path, PathBuf};

pub(super) fn copy_audio_fixtures(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {dst:?}"))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {src:?}"))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "wav" | "flac" | "mp3" | "m4a" | "mp4" | "aac"
        ) {
            continue;
        }
        fs::copy(&path, dst.join(entry.file_name()))
            .with_context(|| format!("copying fixture {:?}", path))?;
    }
    Ok(())
}

pub(super) fn copy_room_eq_fixture(config: &RoomEqConfig, scenario_dir: &Path) -> Result<PathBuf> {
    let source = &config.fixture_dir;
    if !source.is_dir() {
        bail!("RoomEQ fixture does not exist: {}", source.display());
    }

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testkit/roomeq")
        .canonicalize()
        .context("resolving checked-in RoomEQ fixture root")?;
    let source = source
        .canonicalize()
        .with_context(|| format!("resolving RoomEQ fixture {}", source.display()))?;
    if !source.starts_with(&fixture_root) {
        bail!(
            "RoomEQ fixture must be stored under {}, got {}",
            fixture_root.display(),
            source.display()
        );
    }

    let dist_path = config
        .dist_path
        .as_deref()
        .unwrap_or(config.fixture_dir.as_path());
    if dist_path.is_absolute() {
        bail!(
            "RoomEQ dist_path must be relative, got {}",
            dist_path.display()
        );
    }

    let dst = scenario_dir.join("dist").join(dist_path);
    copy_dir_recursive(&source, &dst)?;
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::copy_room_eq_fixture;
    use crate::suite::types::RoomEqConfig;
    use std::path::PathBuf;

    fn config(fixture_dir: PathBuf) -> RoomEqConfig {
        RoomEqConfig {
            fixture_dir,
            dist_path: Some(PathBuf::from("fixtures/roomeq/stereo_reference")),
            target: "NearField".to_string(),
            loss: "Flat".to_string(),
            processing: "Iir".to_string(),
            crossover: "Lr24".to_string(),
            num_filters: 7,
            max_iter: 20,
            population: 24,
            start: false,
            ui_driven: false,
            invalid: None,
        }
    }

    fn checked_in_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testkit/roomeq/stereo_reference")
    }

    #[test]
    fn copies_checked_in_fixture_to_an_isolated_dist_tree() {
        let scenario = tempfile::tempdir().expect("temporary scenario directory");
        let destination = copy_room_eq_fixture(&config(checked_in_fixture()), scenario.path())
            .expect("fixture copy succeeds");

        assert!(destination.starts_with(scenario.path().join("dist")));
        assert!(destination.join("recordings.json").is_file());
        assert!(destination.join("README.md").is_file());
    }

    #[test]
    fn rejects_fixture_outside_checked_in_testkit() {
        let scenario = tempfile::tempdir().expect("temporary scenario directory");
        let error = copy_room_eq_fixture(
            &config(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
            scenario.path(),
        )
        .expect_err("fixture outside testkit must be rejected");

        assert!(error.to_string().contains("must be stored under"));
    }

    #[test]
    fn rejects_absolute_dist_path() {
        let scenario = tempfile::tempdir().expect("temporary scenario directory");
        let mut fixture = config(checked_in_fixture());
        fixture.dist_path = Some(PathBuf::from("/private/tmp/escape"));

        let error = copy_room_eq_fixture(&fixture, scenario.path())
            .expect_err("absolute fixture destination must be rejected");
        assert!(error.to_string().contains("dist_path must be relative"));
    }
}

pub(super) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {dst:?}"))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {src:?}"))? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&path, &dst_path)
                .with_context(|| format!("copying {} to {}", path.display(), dst_path.display()))?;
        }
    }
    Ok(())
}
