#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use sotf_host::MACOS_APP_SANDBOX_HELPER_ENV;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct HelperArgs {
        worker_binary: PathBuf,
        worker_args: Vec<String>,
        worker_env: Vec<(String, String)>,
        forwarded_args: Vec<String>,
    }

    pub fn main() {
        if let Err(err) = run(std::env::args().skip(1)) {
            eprintln!("sotf-macos-sandbox-helper: {err}");
            std::process::exit(1);
        }
    }

    fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
        let args = parse_args(args)?;
        if !args.worker_binary.is_absolute() {
            return Err(format!(
                "external plugin worker path must be absolute: '{}'",
                args.worker_binary.display()
            ));
        }

        let mut command = Command::new(&args.worker_binary);
        command
            .env_clear()
            .env("SOTF_PLUGIN_WORKER", "1")
            .env(MACOS_APP_SANDBOX_HELPER_ENV, "1")
            .args(&args.worker_args)
            .args(&args.forwarded_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (key, value) in &args.worker_env {
            command.env(key, value);
        }

        let status = command
            .spawn()
            .and_then(|mut child| child.wait())
            .map_err(|err| {
                format!(
                    "failed to launch sandboxed external plugin worker '{}': {err}",
                    args.worker_binary.display()
                )
            })?;

        std::process::exit(status.code().unwrap_or(1));
    }

    fn parse_args(args: impl IntoIterator<Item = String>) -> Result<HelperArgs, String> {
        let mut worker_binary = None;
        let mut worker_args = Vec::new();
        let mut worker_env = Vec::new();
        let mut forwarded_args = Vec::new();
        let mut iter = args.into_iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--sandbox-worker-binary" => {
                    worker_binary = Some(PathBuf::from(next_value(
                        "--sandbox-worker-binary",
                        &mut iter,
                    )?));
                }
                "--sandbox-worker-arg" => {
                    worker_args.push(next_value("--sandbox-worker-arg", &mut iter)?);
                }
                "--sandbox-worker-env" => {
                    let entry = next_value("--sandbox-worker-env", &mut iter)?;
                    let (key, value) = entry
                        .split_once('=')
                        .ok_or_else(|| "--sandbox-worker-env must be KEY=VALUE".to_string())?;
                    if key.is_empty() || key.contains('=') {
                        return Err("--sandbox-worker-env key must be non-empty".to_string());
                    }
                    worker_env.push((key.to_string(), value.to_string()));
                }
                _ => forwarded_args.push(arg),
            }
        }

        let worker_binary =
            worker_binary.ok_or_else(|| "missing --sandbox-worker-binary".to_string())?;
        Ok(HelperArgs {
            worker_binary,
            worker_args,
            worker_env,
            forwarded_args,
        })
    }

    fn next_value(
        flag: &'static str,
        iter: &mut impl Iterator<Item = String>,
    ) -> Result<String, String> {
        iter.next()
            .ok_or_else(|| format!("missing value for {flag}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_worker_metadata_and_forwards_worker_args() {
            let args = parse_args([
                "--sandbox-worker-binary".to_string(),
                "/tmp/sotf-worker".to_string(),
                "--sandbox-worker-arg".to_string(),
                "--idle-sleep-micros".to_string(),
                "--sandbox-worker-arg".to_string(),
                "50".to_string(),
                "--sandbox-worker-env".to_string(),
                "SOTF_TEST=value=with=equals".to_string(),
                "--descriptor-json".to_string(),
                "{}".to_string(),
                "--shared-memory".to_string(),
                "/tmp/sotf.shm".to_string(),
            ])
            .unwrap();

            assert_eq!(args.worker_binary, PathBuf::from("/tmp/sotf-worker"));
            assert_eq!(
                args.worker_args,
                vec!["--idle-sleep-micros".to_string(), "50".to_string()]
            );
            assert_eq!(
                args.worker_env,
                vec![("SOTF_TEST".to_string(), "value=with=equals".to_string())]
            );
            assert_eq!(
                args.forwarded_args,
                vec![
                    "--descriptor-json".to_string(),
                    "{}".to_string(),
                    "--shared-memory".to_string(),
                    "/tmp/sotf.shm".to_string(),
                ]
            );
        }

        #[test]
        fn rejects_missing_worker_binary() {
            let err = parse_args(["--descriptor-json".to_string(), "{}".to_string()]).unwrap_err();

            assert!(err.contains("missing --sandbox-worker-binary"));
        }

        #[test]
        fn rejects_invalid_worker_env() {
            let err = parse_args([
                "--sandbox-worker-binary".to_string(),
                "/tmp/sotf-worker".to_string(),
                "--sandbox-worker-env".to_string(),
                "SOTF_TEST".to_string(),
            ])
            .unwrap_err();

            assert!(err.contains("KEY=VALUE"));
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("sotf-macos-sandbox-helper is only supported on macOS");
    std::process::exit(2);
}
