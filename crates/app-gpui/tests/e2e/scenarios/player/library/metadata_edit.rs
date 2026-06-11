use crate::driver::AppDriver;
use crate::factories::{album, stereo_track};
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player::MetadataImportCandidate;
use sotf_audio_player_gpui::app::{InputMode, MetadataEditorState, Screen};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct MetadataEditorScenario;

impl TestScenario for MetadataEditorScenario {
    fn name(&self) -> &'static str {
        "Library Metadata Editor"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        driver.update_app(|app, _| {
            let mut album = album("Original Album")
                .with_year(1999)
                .add_track(stereo_track("Original Track", "Original Artist"))
                .build();
            album.id = Some(42);
            app.library_state.library.albums = vec![album];
            app.invalidate_library_stats();
        });
        driver.navigate_to(Screen::Library);

        driver.update_app(|app, _| {
            let album = app.library_state.library.albums[0].clone();
            app.metadata_editor = Some(MetadataEditorState::for_album(&album).unwrap());
            app.ui_state.input_mode = InputMode::MetadataEditor;
        });
        driver.run_until_parked();

        driver.update_app(|app, _| {
            let editor = app.metadata_editor.as_mut().unwrap();
            editor.fields.title = "Imported Album".to_string();
            editor.search_results.push(MetadataImportCandidate {
                provider_id: "musicbrainz".to_string(),
                provider_entity_id: "release-1".to_string(),
                title: None,
                artist: None,
                album_artist: Some("Imported Artist".to_string()),
                album_title: Some("Imported Album".to_string()),
                year: Some(2024),
                track_number: None,
                disc_number: None,
                isrc: None,
                score: 98,
            });
            let candidate = editor.search_results[0].clone();
            editor.apply_candidate(candidate);
            let patch = editor.patch().unwrap();
            let preview = app
                .library_state
                .preview_metadata_edit(editor.target.clone(), patch)
                .unwrap();
            editor.preview = Some(preview);
        });

        let (mode, title, year, preview_files) = driver.read_app(|app| {
            let editor = app.metadata_editor.as_ref().unwrap();
            (
                app.ui_state.input_mode,
                editor.fields.title.clone(),
                editor.fields.year.clone(),
                editor
                    .preview
                    .as_ref()
                    .map(|preview| preview.affected_files.len())
                    .unwrap_or(0),
            )
        });
        if mode != InputMode::MetadataEditor {
            return Err("Metadata editor did not stay open".into());
        }
        if title != "Imported Album" || year != "2024" || preview_files != 1 {
            return Err("Metadata editor candidate import/preview did not update state".into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_library_metadata_editor_driver_scenario(cx: &mut TestAppContext) {
    let scenario = MetadataEditorScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "metadata editor scenario failed: {:?}",
        result.err()
    );
}
