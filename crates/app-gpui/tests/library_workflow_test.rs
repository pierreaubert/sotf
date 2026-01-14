use gpui::TestAppContext;
use sotf_audio_player_gpui::app::manager::Manager;
use sotf_audio_player_gpui::app::state::library::{
    ChannelFilter, LibraryEvent, LibraryQuery, LibraryResponse, LibrarySortOrder, LibraryState,
};

#[gpui::test]
fn test_library_workflow(cx: &mut TestAppContext) {
    // 1. Setup: Create LibraryState directly (unit testing the manager)
    // We use new_for_test() to avoid database loading in tests
    let mut manager = LibraryState::new_for_test();

    // 2. Test: Search
    manager
        .handle_event(LibraryEvent::SetSearchQuery("Rock".to_string()))
        .unwrap();
    assert_eq!(manager.search_query, "Rock");

    // 3. Test: Clear Search
    manager.handle_event(LibraryEvent::ClearSearch).unwrap();
    assert_eq!(manager.search_query, "");

    // 4. Test: Sorting
    manager
        .handle_event(LibraryEvent::SetSortOrder(LibrarySortOrder::Artist))
        .unwrap();
    assert_eq!(manager.sort_order, LibrarySortOrder::Artist);

    // 5. Test: Filtering
    manager
        .handle_event(LibraryEvent::SetFilter(ChannelFilter::Stereo))
        .unwrap();
    assert_eq!(manager.filter, ChannelFilter::Stereo);

    // 6. Test: Queries
    // Count should be 0 for empty test library
    let response = manager.query(LibraryQuery::ItemCount);
    if let LibraryResponse::Count(count) = response {
        assert_eq!(count, 0);
    } else {
        panic!("Wrong response type for ItemCount");
    }

    // 7. Test: Pagination
    manager.handle_event(LibraryEvent::NextPage).unwrap();
    // Assuming default is 0 and empty library, expecting to stay at 0 or handle gracefully
    assert_eq!(manager.current_page, 0);
}
