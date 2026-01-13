use sotf_audio_player::MusicLibrary;
use std::path::PathBuf;
use std::fs;

mod fixtures;

#[test]
fn test_album_art_discovery_and_thumbnail_generation() {
    fixtures::ensure_demo_files_exist();

    let (_temp_db_dir, db_path) = fixtures::temp_database();
    
    // Create a temporary music directory
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();

    // Copy a demo audio file to the temp music directory
    let demo_file = fixtures::get_demo_file("rock.wav");
    let target_audio = music_path.join("rock.wav");
    fs::copy(&demo_file, &target_audio).unwrap();

    // Copy an image file as 'cover.png'
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_image = root.parent().unwrap().join("app-gpui").join("assets").join("brands").join("focusrite.png");
    let target_image = music_path.join("cover.png");
    
    // Check if source image exists, otherwise skip or use a dummy if needed
    // (In a real environment, we'd ensure a test image is available)
    if src_image.exists() {
        fs::copy(&src_image, &target_image).unwrap();
    } else {
        // Fallback: create a dummy valid PNG if possible, 
        // but let's try to find another one if focusrite.png is missing
        let alt_image = root.join("assets").join("sotf.png");
        if alt_image.exists() {
            fs::copy(&alt_image, &target_image).unwrap();
        } else {
            panic!("Could not find a test image at {:?} or {:?}", src_image, alt_image);
        }
    }

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();

    // Scan the directory - this should find the audio file and the cover.png
    library.scan().expect("Failed to scan directory");

    assert!(!library.albums.is_empty(), "Should have found 1 album");
    let album = &library.albums[0];

    // Verify album art path
    assert!(album.album_art_path.is_some(), "Album art path should be populated");
    assert_eq!(album.album_art_path.as_ref().unwrap().file_name().unwrap(), "cover.png");

    // Verify thumbnail generation
    assert!(album.album_art_thumbnail.is_some(), "Album art thumbnail should be generated");
    assert!(album.album_art_thumbnail.as_ref().unwrap().len() > 0, "Thumbnail should not be empty");

    // Verify it survives database reload
    let mut library2 = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library2.load_from_database().expect("Failed to load from database");
    
    assert!(!library2.albums.is_empty());
    let album2 = &library2.albums[0];
    assert!(album2.album_art_path.is_some());
    assert!(album2.album_art_thumbnail.is_some());
    assert_eq!(album2.album_art_thumbnail, album.album_art_thumbnail);
}

#[test]
fn test_album_art_discovery_in_artwork_subdir() {
    fixtures::ensure_demo_files_exist();

    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();

    // Create 'Artwork' subdirectory
    let artwork_path = music_path.join("Artwork");
    fs::create_dir(&artwork_path).unwrap();

    // Copy audio file
    let demo_file = fixtures::get_demo_file("jazz.wav");
    fs::copy(&demo_file, music_path.join("jazz.wav")).unwrap();

    // Copy image to Artwork/front.png
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_image = root.parent().unwrap().join("app-gpui").join("assets").join("brands").join("focusrite.png");
    let target_image = artwork_path.join("front.png");
    fs::copy(&src_image, &target_image).unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();
    library.scan().expect("Failed to scan directory");

    let album = &library.albums[0];
    assert!(album.album_art_path.is_some());
    assert_eq!(album.album_art_path.as_ref().unwrap().file_name().unwrap(), "front.png");
    assert!(album.album_art_thumbnail.is_some());
}