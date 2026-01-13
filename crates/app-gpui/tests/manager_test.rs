

use gpui::TestAppContext;
use sotf_audio_player_gpui::app::state::app::{App, AppMessage};
use sotf_audio_player_gpui::app::state::library::{LibraryEvent, LibraryQuery, LibraryResponse};
use sotf_audio_player_gpui::app::manager::{Manager, ManagerError};

#[gpui::test]
fn test_manager_protocol(cx: &mut TestAppContext) {
    // 1. Initialize App
    let mut app = App::new();
    
    // 2. Initial State Verification
    assert_eq!(app.library_state.search_query, "", "Search query should be empty initially");

    // 3. Dispatch Event via App::dispatch
    let event = LibraryEvent::SetSearchQuery("jazz".to_string());
    let msg = AppMessage::Library(event);
    
    let result = app.dispatch(msg);
    assert!(result.is_ok(), "Dispatch should succeed");

    // 4. Verify State Change
    assert_eq!(app.library_state.search_query, "jazz", "Search query should be updated");

    // 5. Verify Query
    let count_query = LibraryQuery::ItemCount;
    let response = app.library_state.query(count_query);
    
    if let LibraryResponse::Count(c) = response {
        assert_eq!(c, 0, "Item count should be 0");
    } else {
        panic!("Unexpected response type");
    }
}

