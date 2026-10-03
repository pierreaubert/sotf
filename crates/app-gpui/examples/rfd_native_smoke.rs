//! Exercise the native file dialog backend in a display-required release job.

use std::path::{Path, PathBuf};

fn selected_path(handle: Option<rfd::FileHandle>, expected: &Path) -> anyhow::Result<rfd::FileHandle> {
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
    let mode = args.next().ok_or_else(|| anyhow::anyhow!("mode required"))?;
    let expected = PathBuf::from(args.next().ok_or_else(|| anyhow::anyhow!("path required"))?);
    anyhow::ensure!(args.next().is_none(), "unexpected extra argument");
    let parent = expected.parent().ok_or_else(|| anyhow::anyhow!("fixture parent missing"))?;
    anyhow::ensure!(parent.is_dir(), "fixture parent missing: {}", parent.display());

    smol::block_on(async {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(format!("SOTF RFD {mode}"))
            .set_directory(parent);
        println!("READY SOTF RFD {mode}");
        match mode.as_str() {
            "open" => {
                let file = selected_path(dialog.add_filter("text", &["txt"]).pick_file().await, &expected)?;
                anyhow::ensure!(file.read().await == b"sotf-rfd-fixture\n", "opened file content differs");
            }
            "folder" => {
                selected_path(dialog.pick_folder().await, &expected)?;
            }
            "save" => {
                let name = expected.file_name().ok_or_else(|| anyhow::anyhow!("save filename missing"))?;
                let file = selected_path(
                    dialog.set_file_name(name.to_string_lossy()).save_file().await,
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
    println!("PASS SOTF RFD {mode}");
    Ok(())
}
