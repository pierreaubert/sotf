use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

const APP_LOGS: [&str; 3] = [
    "sotf.stdout.log",
    "sotf.stderr.log",
    // The GPUI binary writes its structured log beneath `--qa`.
    "qa/sotf_gpui_player.log",
];

/// Reject scenario runs that emitted an unapproved panic or ERROR-level log.
///
/// The allowlist intentionally uses exact substrings rather than a broad
/// global suppression: a scenario must document each known environmental
/// message it permits.
pub(super) fn assert_clean_logs(scenario_dir: &Path, allowed_patterns: &[String]) -> Result<()> {
    let mut findings = Vec::new();

    for name in APP_LOGS {
        let path = scenario_dir.join(name);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            // An app that fails before logging initializes still has its
            // stdout/stderr scanned; do not hide that original failure behind
            // a missing optional structured-log file.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).with_context(|| format!("reading {path:?}")),
        };
        for (line_number, line) in String::from_utf8_lossy(&contents).lines().enumerate() {
            if is_unexpected_log(line, allowed_patterns) {
                findings.push(format!("{}:{}: {line}", path.display(), line_number + 1));
            }
        }
    }

    if findings.is_empty() {
        return Ok(());
    }

    let preview = findings.into_iter().take(20).collect::<Vec<_>>().join("\n");
    bail!("unexpected panic/ERROR log output:\n{preview}")
}

/// Known-benign platform teardown noise that is never an app error:
/// wgpu's GL backend reports EGL_BAD_DISPLAY (0x3008) when its
/// context is destroyed after the display at process exit on
/// headless Linux. Exact substrings only, like the allowlist.
const BENIGN_TEARDOWN_PATTERNS: &[&str] = &[
    "EGL 'eglDestroyContext' code 0x3008",
    "EGL 'eglTerminate' code 0x3008",
];

fn is_unexpected_log(line: &str, allowed_patterns: &[String]) -> bool {
    if BENIGN_TEARDOWN_PATTERNS
        .iter()
        .any(|pattern| line.contains(pattern))
    {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    // Match ERROR as a log-level token ("[ts ERROR crate]" from
    // env_logger, or "[ERROR]"), not as a substring: dependency
    // debug lines such as h2's `error=GoAway(.., NO_ERROR, ..)`
    // are not ERROR-level logs.
    let is_failure = line.contains(" ERROR ")
        || line.contains("[ERROR]")
        || lower.contains("panicked at")
        || lower.contains("thread '") && lower.contains("panic");

    is_failure
        && !allowed_patterns
            .iter()
            .any(|pattern| line.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::assert_clean_logs;
    use std::fs;

    #[test]
    fn clean_log_gate_rejects_errors_and_respects_narrow_allowlist() {
        let dir = tempfile::tempdir().expect("temporary scenario directory");
        fs::write(dir.path().join("sotf.stdout.log"), "normal output\n").expect("stdout log");
        fs::write(dir.path().join("sotf.stderr.log"), "normal output\n").expect("stderr log");
        fs::create_dir(dir.path().join("qa")).expect("QA directory");
        fs::write(
            dir.path().join("qa/sotf_gpui_player.log"),
            "[ERROR] unexpected device failure\n",
        )
        .expect("structured log");

        assert!(assert_clean_logs(dir.path(), &[]).is_err());
        assert!(assert_clean_logs(dir.path(), &["unexpected device failure".to_string()]).is_ok());
    }

    #[test]
    fn clean_log_gate_rejects_panics() {
        let dir = tempfile::tempdir().expect("temporary scenario directory");
        fs::write(dir.path().join("sotf.stdout.log"), "normal output\n").expect("stdout log");
        fs::write(
            dir.path().join("sotf.stderr.log"),
            "thread 'main' panicked at src/main.rs:1\n",
        )
        .expect("stderr log");

        assert!(assert_clean_logs(dir.path(), &[]).is_err());
    }
}

#[cfg(test)]
mod no_error_token_tests {
    use super::assert_clean_logs;
    use std::fs;

    #[test]
    fn clean_log_gate_ignores_no_error_debug_lines() {
        let dir = tempfile::tempdir().expect("temporary scenario directory");
        fs::write(
            dir.path().join("sotf.stderr.log"),
            "[2026-11-14T16:20:49Z DEBUG h2::proto::connection] Connection::poll; connection error error=GoAway(\"\", NO_ERROR, Library)\n",
        )
        .expect("stderr log");
        assert!(assert_clean_logs(dir.path(), &[]).is_ok());
    }

    #[test]
    fn clean_log_gate_ignores_wgpu_egl_teardown_noise() {
        let dir = tempfile::tempdir().expect("temporary scenario directory");
        fs::write(
            dir.path().join("sotf.stderr.log"),
            "[2026-11-14T17:34:50Z ERROR wgpu_hal::gles::egl] EGL 'eglDestroyContext' code 0x3008: \n[2026-11-14T17:34:50Z ERROR wgpu_hal::gles::egl] EGL 'eglTerminate' code 0x3008: \n",
        )
        .expect("stderr log");
        assert!(assert_clean_logs(dir.path(), &[]).is_ok());
    }
}
