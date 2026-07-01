use sotf_audio_player::MusicLibrary;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

mod fixtures;

fn write_minimal_wav(path: &std::path::Path) {
    let channels = 2u16;
    let sample_rate = 44_100u32;
    let bits_per_sample = 16u16;
    let samples_per_channel = 8u32;
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let data_size = samples_per_channel * channels as u32 * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample;
    let block_align = channels * (bits_per_sample / 8);

    let mut file = std::fs::File::create(path).expect("create wav");
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();
    file.write_all(&vec![0u8; data_size as usize]).unwrap();
}

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
    let src_image = root
        .parent()
        .unwrap()
        .join("app-gpui")
        .join("assets")
        .join("brands")
        .join("focusrite.png");
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
            panic!(
                "Could not find a test image at {:?} or {:?}",
                src_image, alt_image
            );
        }
    }

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();

    // Scan the directory - this should find the audio file and the cover.png
    library.scan().expect("Failed to scan directory");

    assert!(!library.albums.is_empty(), "Should have found 1 album");
    let album = &library.albums[0];

    // Verify album art path
    assert!(
        album.album_art_path.is_some(),
        "Album art path should be populated"
    );
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "cover.png"
    );

    // Verify thumbnail generation
    assert!(
        album.album_art_thumbnail.is_some(),
        "Album art thumbnail should be generated"
    );
    assert!(
        !album.album_art_thumbnail.as_ref().unwrap().is_empty(),
        "Thumbnail should not be empty"
    );

    // Verify it survives database reload
    let mut library2 = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library2
        .load_from_database()
        .expect("Failed to load from database");

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
    let src_image = root
        .parent()
        .unwrap()
        .join("app-gpui")
        .join("assets")
        .join("brands")
        .join("focusrite.png");
    let target_image = artwork_path.join("front.png");
    fs::copy(&src_image, &target_image).unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();
    library.scan().expect("Failed to scan directory");

    let album = &library.albums[0];
    assert!(album.album_art_path.is_some());
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "front.png"
    );
    assert!(album.album_art_thumbnail.is_some());
}

#[test]
fn test_album_art_discovered_on_incremental_rescan() {
    fixtures::ensure_demo_files_exist();

    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();
    let disc_path = music_path.join("CD1");
    fs::create_dir(&disc_path).unwrap();

    // Create audio with no embedded artwork.
    write_minimal_wav(&disc_path.join("rock.wav"));

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();
    library.scan().expect("Failed to scan directory");

    let album = &library.albums[0];
    assert!(
        album.album_art_path.is_none(),
        "Album should have no artwork before cover.jpg is added"
    );

    // Now add cover.jpg to the album directory and rescan incrementally
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_image = root
        .parent()
        .unwrap()
        .join("app-gpui")
        .join("assets")
        .join("brands")
        .join("focusrite.png");
    let target_image = music_path.join("cover.jpg");
    fs::copy(&src_image, &target_image).unwrap();

    library
        .scan_incremental(true)
        .expect("Failed to rescan directory incrementally");

    let album = &library.albums[0];
    assert!(
        album.album_art_path.is_some(),
        "Incremental rescan should discover the newly added cover.jpg"
    );
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "cover.jpg"
    );
    assert!(album.album_art_thumbnail.is_some());

    let mut reloaded = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    reloaded
        .load_from_database()
        .expect("Failed to reload library");
    let reloaded_album = &reloaded.albums[0];
    assert_eq!(
        reloaded_album
            .album_art_path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap(),
        "cover.jpg"
    );
    assert!(reloaded_album.album_art_thumbnail.is_some());
}

#[test]
fn test_album_art_at_album_root_with_tracks_in_subdir() {
    fixtures::ensure_demo_files_exist();

    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();
    let disc_dir = music_path.join("CD1");
    fs::create_dir(&disc_dir).unwrap();

    // Copy audio file into a disc subdirectory, cover.jpg at the album root
    let demo_file = fixtures::get_demo_file("rock.wav");
    fs::copy(&demo_file, disc_dir.join("rock.wav")).unwrap();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_image = root
        .parent()
        .unwrap()
        .join("app-gpui")
        .join("assets")
        .join("brands")
        .join("focusrite.png");
    let target_image = music_path.join("cover.jpg");
    fs::copy(&src_image, &target_image).unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();
    library.scan().expect("Failed to scan directory");

    assert!(!library.albums.is_empty(), "Should have found 1 album");
    let album = &library.albums[0];
    assert!(
        album.album_art_path.is_some(),
        "Should find cover.jpg in album root even when tracks are in a subdirectory"
    );
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "cover.jpg"
    );
}

#[test]
fn test_album_art_prefers_new_cover_on_rescan() {
    fixtures::ensure_demo_files_exist();

    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();

    let demo_file = fixtures::get_demo_file("rock.wav");
    fs::copy(&demo_file, music_path.join("rock.wav")).unwrap();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_image = root
        .parent()
        .unwrap()
        .join("app-gpui")
        .join("assets")
        .join("brands")
        .join("focusrite.png");

    // Initial scan finds a booklet front image
    let front_path = music_path.join("booklet_front.png");
    fs::copy(&src_image, &front_path).unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();
    library.scan().expect("Failed to scan directory");

    let album = &library.albums[0];
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "booklet_front.png"
    );

    // Add a proper cover.jpg and rescan incrementally
    let cover_path = music_path.join("cover.jpg");
    fs::copy(&src_image, &cover_path).unwrap();

    library
        .scan_incremental(true)
        .expect("Failed to rescan directory incrementally");

    let album = &library.albums[0];
    assert_eq!(
        album.album_art_path.as_ref().unwrap().file_name().unwrap(),
        "cover.jpg",
        "Rescan should prefer the newly added cover.jpg"
    );
}
