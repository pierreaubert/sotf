// ============================================================================
// Stable Identity Generation
// ============================================================================
//
// Deterministic UUID v5 generation from normalized album/track metadata.
// Two independent SOTF instances scanning the same music produce identical UUIDs,
// enabling P2P merge without prior coordination.

use uuid::Uuid;

/// SOTF-specific UUID v5 namespace (generated once, never changes).
/// This is a UUID v4 used as the namespace for all SOTF UUID v5 generation.
const SOTF_NAMESPACE: Uuid = Uuid::from_bytes([
    0x8a, 0x3b, 0x5c, 0x7d, 0x2e, 0x4f, 0x6a, 0x1b, 0x9c, 0x0d, 0x3e, 0x5f, 0x7a, 0x8b, 0x1c,
    0x2d,
]);

/// Generate a stable UUID for an album from normalized metadata.
///
/// The UUID is deterministic: same `(artist, title)` always produces the same UUID.
pub fn album_uuid(normalized_artist: &str, normalized_title: &str) -> Uuid {
    let input = format!("{}\0{}", normalized_artist, normalized_title);
    Uuid::new_v5(&SOTF_NAMESPACE, input.as_bytes())
}

/// Generate a stable UUID for a track from its album UUID and normalized metadata.
///
/// The UUID is deterministic: same `(album_uuid, title, disc, track_number)` always
/// produces the same UUID.
pub fn track_uuid(
    album_uuid: &Uuid,
    normalized_title: &str,
    disc_number: u32,
    track_number: u32,
) -> Uuid {
    let input = format!(
        "{}\0{}\0{}\0{}",
        album_uuid, normalized_title, disc_number, track_number
    );
    Uuid::new_v5(&SOTF_NAMESPACE, input.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_album_uuid_deterministic() {
        let u1 = album_uuid("pink floyd", "the wall");
        let u2 = album_uuid("pink floyd", "the wall");
        assert_eq!(u1, u2);
    }

    #[test]
    fn test_album_uuid_different_for_different_albums() {
        let u1 = album_uuid("pink floyd", "the wall");
        let u2 = album_uuid("pink floyd", "wish you were here");
        assert_ne!(u1, u2);
    }

    #[test]
    fn test_album_uuid_different_for_different_artists() {
        let u1 = album_uuid("pink floyd", "the wall");
        let u2 = album_uuid("led zeppelin", "the wall");
        assert_ne!(u1, u2);
    }

    #[test]
    fn test_track_uuid_deterministic() {
        let au = album_uuid("pink floyd", "the wall");
        let t1 = track_uuid(&au, "comfortably numb", 2, 6);
        let t2 = track_uuid(&au, "comfortably numb", 2, 6);
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_track_uuid_different_disc() {
        let au = album_uuid("pink floyd", "the wall");
        let t1 = track_uuid(&au, "comfortably numb", 1, 6);
        let t2 = track_uuid(&au, "comfortably numb", 2, 6);
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_track_uuid_different_track_number() {
        let au = album_uuid("pink floyd", "the wall");
        let t1 = track_uuid(&au, "comfortably numb", 2, 5);
        let t2 = track_uuid(&au, "comfortably numb", 2, 6);
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_uuid_is_v5() {
        let u = album_uuid("pink floyd", "the wall");
        assert_eq!(u.get_version_num(), 5);
    }
}
