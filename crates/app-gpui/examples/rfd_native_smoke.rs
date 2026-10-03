//! Exercise the native file dialog backend in a display-required release job.

use std::path::{Path, PathBuf};

fn selected_path(
    handle: Option<rfd::FileHandle>,
    expected: &Path,
) -> anyhow::Result<rfd::FileHandle> {
    let handle = handle.ok_or_else(|| anyhow::anyhow!("dialog returned no selection"))?;
    anyhow::ensure!(
        handle.path() == expected,
        "selected {}, expected {}",
        handle.path().display(),
        expected.display()
    );
    Ok(handle)
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("mode required"))?;
    let expected = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("path required"))?,
    );
    anyhow::ensure!(args.next().is_none(), "unexpected extra argument");
    let parent = expected
        .parent()
        .ok_or_else(|| anyhow::anyhow!("fixture parent missing"))?;
    anyhow::ensure!(
        parent.is_dir(),
        "fixture parent missing: {}",
        parent.display()
    );

    #[cfg(target_os = "macos")]
    return macos::run(mode, expected);

    #[cfg(not(target_os = "macos"))]
    smol::block_on(async {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(format!("SOTF RFD {mode}"))
            .set_directory(parent);
        println!("READY SOTF RFD {mode}");
        match mode.as_str() {
            "open" => {
                let file = selected_path(
                    dialog.add_filter("text", &["txt"]).pick_file().await,
                    &expected,
                )?;
                anyhow::ensure!(
                    file.read().await == b"sotf-rfd-fixture\n",
                    "opened file content differs"
                );
            }
            "folder" => {
                selected_path(dialog.pick_folder().await, &expected)?;
            }
            "save" => {
                let name = expected
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("save filename missing"))?;
                let file = selected_path(
                    dialog
                        .set_file_name(name.to_string_lossy())
                        .save_file()
                        .await,
                    &expected,
                )?;
                file.write(b"sotf-rfd-saved\n").await?;
                anyhow::ensure!(
                    std::fs::read(&expected)? == b"sotf-rfd-saved\n",
                    "saved file content differs"
                );
            }
            "cancel" => {
                anyhow::ensure!(dialog.pick_file().await.is_none(), "cancel returned a file");
            }
            _ => anyhow::bail!("unknown mode: {mode}"),
        }
        Ok::<(), anyhow::Error>(())
    })?;
    #[cfg(not(target_os = "macos"))]
    {
        println!("PASS SOTF RFD {mode}");
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{PathBuf, selected_path};
    use gpui::*;
    use std::sync::{Arc, Mutex};

    struct SmokeView;

    impl Render for SmokeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(0xffffff))
        }
    }

    async fn check_dialog(
        dialog: rfd::AsyncFileDialog,
        mode: &str,
        expected: &std::path::Path,
    ) -> anyhow::Result<()> {
        match mode {
            "open" => {
                let file = selected_path(
                    dialog.add_filter("text", &["txt"]).pick_file().await,
                    expected,
                )?;
                anyhow::ensure!(
                    file.read().await == b"sotf-rfd-fixture\n",
                    "opened file content differs"
                );
            }
            "folder" => {
                selected_path(dialog.pick_folder().await, expected)?;
            }
            "save" => {
                let name = expected
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("save filename missing"))?;
                let file = selected_path(
                    dialog
                        .set_file_name(name.to_string_lossy())
                        .save_file()
                        .await,
                    expected,
                )?;
                file.write(b"sotf-rfd-saved\n").await?;
                anyhow::ensure!(
                    std::fs::read(expected)? == b"sotf-rfd-saved\n",
                    "saved file content differs"
                );
            }
            "cancel" => {
                anyhow::ensure!(dialog.pick_file().await.is_none(), "cancel returned a file");
            }
            _ => anyhow::bail!("unknown mode: {mode}"),
        }
        Ok(())
    }

    pub(super) fn run(mode: String, expected: PathBuf) -> anyhow::Result<()> {
        let result = Arc::new(Mutex::new(None));
        let completed = result.clone();
        let pass_mode = mode.clone();
        let platform = std::rc::Rc::new(gpui_macos::MacPlatform::new(false));
        gpui::Application::with_platform(platform).run(move |cx| {
            let mode_for_window = mode.clone();
            let result_for_window = completed.clone();
            if let Err(error) = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("SOTF native dialog smoke parent".into()),
                        ..Default::default()
                    }),
                    show: true,
                    focus: true,
                    ..Default::default()
                },
                move |window, cx| {
                    let dialog = rfd::AsyncFileDialog::new()
                        .set_title(format!("SOTF RFD {mode_for_window}"))
                        .set_directory(expected.parent().expect("validated fixture parent"))
                        .set_parent(window);
                    cx.new(move |cx| {
                        cx.spawn(async move |_, cx| {
                            println!("READY SOTF RFD {mode_for_window}");
                            let outcome = check_dialog(dialog, &mode_for_window, &expected).await;
                            *result_for_window.lock().expect("smoke result lock") = Some(outcome);
                            cx.update(|cx| cx.quit());
                        })
                        .detach();
                        SmokeView
                    })
                },
            ) {
                *completed.lock().expect("smoke result lock") = Some(Err(error.into()));
                cx.quit();
            }
        });
        let outcome = result
            .lock()
            .expect("smoke result lock")
            .take()
            .ok_or_else(|| anyhow::anyhow!("native dialog app quit before a result"))?;
        outcome?;
        println!("PASS SOTF RFD {pass_mode}");
        Ok(())
    }
}
