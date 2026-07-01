use super::album::Album;
use super::find::find_album_art;
use super::is::is_image_file;
use super::misc::clean_album_title;
use super::music_library::MusicLibrary;
use super::normalize::normalize_album_key;
use super::track::Track;
use std::collections::HashMap;
use std::path::PathBuf;

mod misc;

#[test]
fn test_library_creation() {
    let lib = MusicLibrary::new();
    assert_eq!(lib.directories.len(), 0);
    assert_eq!(lib.albums.len(), 0);
}

#[test]
fn test_add_remove_directory() {
    let mut lib = MusicLibrary::new();
    let path = PathBuf::from("/tmp/music");

    let result = lib.add_directory(path.clone());
    assert!(result.is_ok());
    assert_eq!(lib.directories.len(), 1);

    lib.remove_directory(0);
    assert_eq!(lib.directories.len(), 0);
}

#[test]
fn test_add_directory_subtree_detection() {
    let mut lib = MusicLibrary::new();

    // Add parent directory
    let parent = PathBuf::from("/tmp/music");
    assert!(lib.add_directory(parent.clone()).is_ok());
    assert_eq!(lib.directories.len(), 1);

    // Try to add a subdirectory - should fail
    let child = PathBuf::from("/tmp/music/jazz");
    let result = lib.add_directory(child);
    assert!(result.is_err());
    assert_eq!(lib.directories.len(), 1); // Still just the parent

    // Add a sibling directory - should succeed
    let sibling = PathBuf::from("/tmp/videos");
    assert!(lib.add_directory(sibling).is_ok());
    assert_eq!(lib.directories.len(), 2);
}

#[test]
fn test_search_albums_empty_library() {
    let lib = MusicLibrary::new();
    let results = lib.search_albums("test");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_search_albums_matches_directory_and_channel_label() {
    let mut lib = MusicLibrary::new();

    lib.albums.push(Album {
        id: None,
        title: "Ace Cool".to_string(),
        year: None,
        tracks: vec![Track {
            path: PathBuf::from(
                "/Volumes/home_ext1/Music/sotf-qa/ace1.5/album16flac-5.0/track.flac",
            ),
            title: Some("Upmixed".to_string()),
            channels: Some(5),
            ..Default::default()
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    });

    let results = lib.search_albums("album16flac-5.0");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Ace Cool");

    let results = lib.search_albums("5.0");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Ace Cool");
}

#[test]
fn test_load_directories_from_database() {
    use crate::database::MusicDatabase;
    use tempfile::TempDir;

    // Create a temporary directory for the test database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_music.db");

    // Create and populate database (using test-only method)
    {
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();

        // Simulate a scan by recording scan history
        db.record_scan(&PathBuf::from("/music/rock"), 50, 5)
            .unwrap();
        db.record_scan(&PathBuf::from("/music/jazz"), 30, 3)
            .unwrap();
    }

    // Create library with the test database
    let mut lib = MusicLibrary {
        directories: Vec::new(),
        albums: Vec::new(),
        db: Some(MusicDatabase::open_for_testing(&db_path).unwrap()),
        dir_stats_cache: HashMap::new(),
    };

    // Load from database
    lib.load_from_database().unwrap();

    // Note: directories will only be loaded if they exist on disk
    // In this test, they don't exist, so directories should be empty
    // In a real scenario where the paths exist, they would be loaded
    assert_eq!(lib.directories.len(), 0); // Paths don't exist
}

#[test]
fn test_load_directories_filters_subtrees() {
    use crate::database::MusicDatabase;
    use tempfile::TempDir;

    // Create actual temporary directories on disk
    let temp_root = TempDir::new().unwrap();
    let parent_dir = temp_root.path().join("music");
    let child_dir = parent_dir.join("rock");

    std::fs::create_dir_all(&parent_dir).unwrap();
    std::fs::create_dir_all(&child_dir).unwrap();

    // Create database (using test-only method)
    let db_path = temp_root.path().join("test_music.db");
    {
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();

        // Record scans for both parent and child
        db.record_scan(&parent_dir, 100, 10).unwrap();
        db.record_scan(&child_dir, 50, 5).unwrap();
    }

    // Create library and load from database
    let mut lib = MusicLibrary {
        directories: Vec::new(),
        albums: Vec::new(),
        db: Some(MusicDatabase::open_for_testing(&db_path).unwrap()),
        dir_stats_cache: HashMap::new(),
    };

    lib.load_from_database().unwrap();

    // Should only have 1 directory (the parent), not the child
    assert_eq!(lib.directories.len(), 1);

    // Verify it's the parent directory
    let canonical_parent = parent_dir.canonicalize().unwrap();
    let loaded_path = lib.directories[0].path.canonicalize().unwrap();
    assert_eq!(loaded_path, canonical_parent);
}

#[test]
fn test_clean_album_title() {
    // Basic cases
    assert_eq!(clean_album_title("Album Title"), "Album Title");
    assert_eq!(clean_album_title("Album Title (CD 1)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (CD-1)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (CD - 1)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (Disc 1)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (Disc-1)"), "Album Title");

    // Case insensitivity
    assert_eq!(clean_album_title("Album Title (cd 1)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (DISC 1)"), "Album Title");

    // Without parentheses
    assert_eq!(clean_album_title("Album Title CD 1"), "Album Title");
    assert_eq!(clean_album_title("Album Title Disc 1"), "Album Title");
    assert_eq!(clean_album_title("Album Title Vol. 1"), "Album Title");

    // User reported cases
    assert_eq!(
        clean_album_title("After The Fall (CD - 2)"),
        "After The Fall"
    );
    assert_eq!(clean_album_title("After The Fall (CD-1)"), "After The Fall");
    assert_eq!(
        clean_album_title("A Night On The Town(CD 1)"),
        "A Night On The Town"
    );
    assert_eq!(
        clean_album_title("A Night On The Town [CD 1]"),
        "A Night On The Town"
    );
    assert_eq!(
        clean_album_title("A Night On The Town[CD 1]"),
        "A Night On The Town"
    );

    // No space before number (CD1, Disc2)
    assert_eq!(clean_album_title("ALPHA & OMEGA CD1"), "ALPHA & OMEGA");
    assert_eq!(clean_album_title("Alpha & Omega CD2"), "Alpha & Omega");
    assert_eq!(clean_album_title("Album Title CD1"), "Album Title");
    assert_eq!(clean_album_title("Album Title Disc2"), "Album Title");
    assert_eq!(clean_album_title("Album Title (CD1)"), "Album Title");

    // Catalog numbers in parentheses
    assert_eq!(
        clean_album_title("A Night On The Town (3116-2)"),
        "A Night On The Town"
    );
    assert_eq!(
        clean_album_title("A Night On The Town (R2 47730)"),
        "A Night On The Town"
    );
    assert_eq!(clean_album_title("Album Title (ABC-12345)"), "Album Title");
    assert_eq!(clean_album_title("Album Title (MFSL 1234)"), "Album Title");

    // Catalog numbers in square brackets
    assert_eq!(clean_album_title("Passion [RWCD 1]"), "Passion");
    assert_eq!(
        clean_album_title("Shaking The Tree [PGCD 7]"),
        "Shaking The Tree"
    );
    assert_eq!(
        clean_album_title("Us [PGCD 7] - Digipack"),
        "Us [PGCD 7] - Digipack" // not at end, so not stripped
    );
    assert_eq!(clean_album_title("Album Title [ABC-123]"), "Album Title");

    // Should NOT clean
    assert_eq!(clean_album_title("AC/DC"), "AC/DC");
    assert_eq!(clean_album_title("Disco Volante"), "Disco Volante");
    assert_eq!(clean_album_title("The CD Is Dead"), "The CD Is Dead");
    assert_eq!(clean_album_title("Album (Live)"), "Album (Live)");
    assert_eq!(
        clean_album_title("Album (Remastered)"),
        "Album (Remastered)"
    );
    assert_eq!(
        clean_album_title("Album (Deluxe Edition)"),
        "Album (Deluxe Edition)"
    );
}

#[test]
fn test_normalize_album_key() {
    // Test basic normalization
    assert_eq!(
        normalize_album_key("2Cellos"),
        normalize_album_key("2CELLOS")
    );
    assert_eq!(
        normalize_album_key("2Cellos"),
        normalize_album_key("2 Cellos ")
    );

    // Test diacritics removal
    assert_eq!(normalize_album_key("Café"), "cafe");
    assert_eq!(normalize_album_key("Naïve"), "naive");
    assert_eq!(normalize_album_key("Björk"), "bjork");
    assert_eq!(normalize_album_key("Señor"), "senor");

    // Test special character removal
    assert_eq!(normalize_album_key("The Beatles!"), "thebeatles");
    assert_eq!(normalize_album_key("AC/DC"), "acdc");
    assert_eq!(normalize_album_key("Album: Title"), "albumtitle");
    assert_eq!(normalize_album_key("The Album, Vol. 2"), "thealbumvol.2");
    assert_eq!(normalize_album_key("Rock & Roll"), "rockroll");

    // Test that periods are kept
    assert_eq!(normalize_album_key("Vol. 2"), "vol.2");
    assert_eq!(normalize_album_key("U.S.A."), "u.s.a.");

    // Test numbers are kept
    assert_eq!(normalize_album_key("2Pac"), "2pac");
    assert_eq!(normalize_album_key("Album 123"), "album123");

    // Test UTF-8 letters and numbers are kept
    assert_eq!(normalize_album_key("日本語"), "日本語");
    assert_eq!(normalize_album_key("Москва"), "москва");
    assert_eq!(normalize_album_key("Αθήνα"), "αθηνα");
}

#[test]
fn test_is_image_file() {
    // Valid image extensions
    assert!(is_image_file(&PathBuf::from("cover.jpg")));
    assert!(is_image_file(&PathBuf::from("cover.jpeg")));
    assert!(is_image_file(&PathBuf::from("cover.JPG")));
    assert!(is_image_file(&PathBuf::from("cover.png")));
    assert!(is_image_file(&PathBuf::from("cover.PNG")));
    assert!(is_image_file(&PathBuf::from("cover.gif")));
    assert!(is_image_file(&PathBuf::from("cover.webp")));

    // Invalid extensions
    assert!(!is_image_file(&PathBuf::from("track.flac")));
    assert!(!is_image_file(&PathBuf::from("track.mp3")));
    assert!(!is_image_file(&PathBuf::from("readme.txt")));
    assert!(!is_image_file(&PathBuf::from("no_extension")));
}

#[test]
fn test_find_album_art_common_names() {
    use std::fs::File;
    use tempfile::TempDir;

    // Create temporary directory with a common album art filename
    let temp_dir = TempDir::new().unwrap();
    let cover_path = temp_dir.path().join("cover.jpg");
    File::create(&cover_path).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), cover_path);
}

#[test]
fn test_find_album_art_front_in_name() {
    use std::fs::File;
    use tempfile::TempDir;

    // Create temporary directory with a file containing "front" in the name
    let temp_dir = TempDir::new().unwrap();
    let front_path = temp_dir.path().join("booklet_front.jpg");
    let back_path = temp_dir.path().join("booklet_back.jpg");
    File::create(&front_path).unwrap();
    File::create(&back_path).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), front_path);
}

#[test]
fn test_find_album_art_single_image() {
    use std::fs::File;
    use tempfile::TempDir;

    // Create temporary directory with only one image file
    let temp_dir = TempDir::new().unwrap();
    let image_path = temp_dir.path().join("some_random_name.jpg");
    File::create(&image_path).unwrap();
    // Add a non-image file
    File::create(temp_dir.path().join("track.flac")).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), image_path);
}

#[test]
fn test_find_album_art_in_artwork_subdir() {
    use std::fs::{File, create_dir};
    use tempfile::TempDir;

    // Create temporary directory with an "Artwork" subdirectory
    let temp_dir = TempDir::new().unwrap();
    let artwork_dir = temp_dir.path().join("Artwork");
    create_dir(&artwork_dir).unwrap();
    let cover_path = artwork_dir.join("cover.jpg");
    File::create(&cover_path).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), cover_path);
}

#[test]
fn test_find_album_art_in_covers_subdir_lowercase() {
    use std::fs::{File, create_dir};
    use tempfile::TempDir;

    // Create temporary directory with a "covers" subdirectory (lowercase)
    let temp_dir = TempDir::new().unwrap();
    let covers_dir = temp_dir.path().join("covers");
    create_dir(&covers_dir).unwrap();
    let front_path = covers_dir.join("front.png");
    File::create(&front_path).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), front_path);
}

#[test]
fn test_find_album_art_no_images() {
    use std::fs::File;
    use tempfile::TempDir;

    // Create temporary directory with only audio files
    let temp_dir = TempDir::new().unwrap();
    File::create(temp_dir.path().join("track1.flac")).unwrap();
    File::create(temp_dir.path().join("track2.flac")).unwrap();

    let result = find_album_art(temp_dir.path());
    assert!(result.is_none());
}

#[test]
fn test_generate_thumbnail_handles_misnamed_jpeg() {
    use std::fs::File;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let cover_path = temp_dir.path().join("cover.png");

    // Write a JPEG-encoded image to a file with a .png extension.
    let img = image::DynamicImage::new_rgb8(4, 4);
    let mut file = File::create(&cover_path).unwrap();
    img.write_to(&mut file, image::ImageFormat::Jpeg).unwrap();

    let thumbnail = super::consts::generate_thumbnail(&cover_path);
    assert!(
        thumbnail.is_some(),
        "thumbnail generation should succeed even when extension does not match content"
    );
    assert!(thumbnail.unwrap().starts_with(b"\x89PNG"));
}

#[test]
fn test_generate_thumbnail_quarantines_corrupt_image() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let cover_path = temp_dir.path().join("cover.jpg");

    // Write bytes that are not a valid JPEG.
    fs::write(&cover_path, b"not a valid jpeg").unwrap();

    let thumbnail = super::consts::generate_thumbnail(&cover_path);
    assert!(
        thumbnail.is_none(),
        "thumbnail generation should fail for corrupt data"
    );
    assert!(
        !cover_path.exists(),
        "corrupt cover file should be moved away"
    );

    let bak_path = temp_dir.path().join("cover.jpg.bak");
    assert!(
        bak_path.exists(),
        "corrupt cover file should be quarantined as cover.jpg.bak"
    );
}

#[test]
fn test_clean_album_title_edge_cases() {
    // Empty string
    assert_eq!(clean_album_title(""), "");

    // Only disc marker
    assert_eq!(clean_album_title("(CD 1)"), "");

    // Volume marker
    assert_eq!(clean_album_title("Album Vol. 2"), "Album");
    assert_eq!(clean_album_title("Album vol 1"), "Album");

    // Disc in middle of title should not be stripped
    assert_eq!(clean_album_title("The CD Is Dead"), "The CD Is Dead");

    // Multiple parentheses - should strip the last catalog number
    assert_eq!(clean_album_title("Album (Live) (123-4)"), "Album (Live)");

    // Bracket catalog number that is also a disc marker (should NOT strip)
    assert_eq!(clean_album_title("Album [CD 1]"), "Album");

    // Short catalog number in parentheses
    assert_eq!(clean_album_title("Album (R2 47730)"), "Album");

    // Case-insensitive disc markers
    assert_eq!(clean_album_title("Album (cd 1)"), "Album");
    assert_eq!(clean_album_title("Album (DISC 2)"), "Album");
}
