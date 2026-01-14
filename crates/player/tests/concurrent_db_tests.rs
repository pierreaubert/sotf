use sotf_audio_player::Album;
use sotf_audio_player::database::MusicDatabase;
use std::sync::{Arc, Barrier};
use std::thread;

mod fixtures;

#[test]
fn test_multiple_concurrent_writers() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    // Initial database creation to set up schema
    {
        let _db = MusicDatabase::open_for_testing(&db_path).unwrap();
    }

    let num_writers = 4;
    let iterations = 20;
    let barrier = Arc::new(Barrier::new(num_writers + 1));
    let mut handles = vec![];

    for w in 0..num_writers {
        let db_path_clone = db_path.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            let mut db = MusicDatabase::open_for_testing(&db_path_clone).unwrap();
            barrier_clone.wait();

            let mut errors = 0;
            for i in 0..iterations {
                let album = Album {
                    id: None,
                    title: format!("Writer {} Album {}", w, i),
                    year: Some(2000 + i),
                    tracks: vec![],
                    album_art_path: None,
                    album_art_thumbnail: None,
                    play_count: 0,
                    edition: None,
                    dynamic_range: None,
                };

                if let Err(e) = db.save_albums(&[album]) {
                    errors += 1;
                    println!("Writer {} error: {}", w, e);
                }
                // Small sleep to allow other writers in
                thread::sleep(std::time::Duration::from_millis(1));
            }
            errors
        });
        handles.push(handle);
    }

    // Reader thread
    let db_path_clone = db_path.clone();
    let barrier_clone = barrier.clone();
    let reader_handle = thread::spawn(move || {
        let db = MusicDatabase::open_for_testing(&db_path_clone).unwrap();
        barrier_clone.wait();

        let mut total_albums_seen = 0;
        for _ in 0..100 {
            if let Ok(albums) = db.load_library() {
                total_albums_seen = total_albums_seen.max(albums.len());
            }
            thread::sleep(std::time::Duration::from_millis(1));
        }
        total_albums_seen
    });

    let total_errors: i32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let max_albums = reader_handle.join().unwrap();

    println!("Total writer errors: {}", total_errors);
    println!("Max albums seen by reader: {}", max_albums);

    // If we have errors, it's likely due to "database is locked"
    // In a production app, we'd want this to be 0
    assert_eq!(
        total_errors, 0,
        "Should have no writer errors after enabling WAL and busy timeout"
    );
}
