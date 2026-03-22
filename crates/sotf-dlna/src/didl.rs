// ============================================================================
// DIDL-Lite XML generation for DLNA content browsing
// ============================================================================
//
// DIDL-Lite is the XML format used by UPnP ContentDirectory to describe
// media items (tracks, albums, containers/folders).

/// A media item for DIDL-Lite serialization.
#[derive(Debug, Clone)]
pub struct DidlItem {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Option<f64>,
    pub resource_url: String,
    pub mime_type: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_depth: Option<u32>,
    pub file_size: Option<u64>,
}

/// A container (folder/album) for DIDL-Lite serialization.
#[derive(Debug, Clone)]
pub struct DidlContainer {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub child_count: u32,
}

/// Generate DIDL-Lite XML for a set of items and containers.
pub fn didl_lite(containers: &[DidlContainer], items: &[DidlItem]) -> String {
    let mut xml = String::from(
        r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">"#,
    );

    for c in containers {
        xml.push_str(&format!(
            r#"<container id="{id}" parentID="{parent}" restricted="1" childCount="{count}"><dc:title>{title}</dc:title><upnp:class>object.container.album.musicAlbum</upnp:class></container>"#,
            id = xml_escape(&c.id),
            parent = xml_escape(&c.parent_id),
            count = c.child_count,
            title = xml_escape(&c.title),
        ));
    }

    for item in items {
        let mut res_attrs = format!(
            r#"protocolInfo="http-get:*:{}:*""#,
            xml_escape(&item.mime_type)
        );
        if let Some(dur) = item.duration {
            let h = (dur as u64) / 3600;
            let m = ((dur as u64) % 3600) / 60;
            let s = dur % 60.0;
            res_attrs.push_str(&format!(r#" duration="{:02}:{:02}:{:06.3}""#, h, m, s));
        }
        if let Some(sr) = item.sample_rate {
            res_attrs.push_str(&format!(r#" sampleFrequency="{}""#, sr));
        }
        if let Some(ch) = item.channels {
            res_attrs.push_str(&format!(r#" nrAudioChannels="{}""#, ch));
        }
        if let Some(bd) = item.bit_depth {
            res_attrs.push_str(&format!(r#" bitsPerSample="{}""#, bd));
        }
        if let Some(fs) = item.file_size {
            res_attrs.push_str(&format!(r#" size="{}""#, fs));
        }

        xml.push_str(&format!(
            r#"<item id="{id}" parentID="{parent}" restricted="1"><dc:title>{title}</dc:title>"#,
            id = xml_escape(&item.id),
            parent = xml_escape(&item.parent_id),
            title = xml_escape(&item.title),
        ));
        if let Some(ref artist) = item.artist {
            xml.push_str(&format!("<dc:creator>{}</dc:creator>", xml_escape(artist)));
            xml.push_str(&format!(
                "<upnp:artist>{}</upnp:artist>",
                xml_escape(artist)
            ));
        }
        if let Some(ref album) = item.album {
            xml.push_str(&format!("<upnp:album>{}</upnp:album>", xml_escape(album)));
        }
        if let Some(ref genre) = item.genre {
            xml.push_str(&format!("<upnp:genre>{}</upnp:genre>", xml_escape(genre)));
        }
        if let Some(track) = item.track_number {
            xml.push_str(&format!(
                "<upnp:originalTrackNumber>{}</upnp:originalTrackNumber>",
                track
            ));
        }
        xml.push_str("<upnp:class>object.item.audioItem.musicTrack</upnp:class>");
        xml.push_str(&format!(
            "<res {}>{}</res>",
            res_attrs,
            xml_escape(&item.resource_url)
        ));
        xml.push_str("</item>");
    }

    xml.push_str("</DIDL-Lite>");
    xml
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_didl_item() {
        let items = vec![DidlItem {
            id: "track-1".to_string(),
            parent_id: "album-1".to_string(),
            title: "Comfortably Numb".to_string(),
            artist: Some("Pink Floyd".to_string()),
            album: Some("The Wall".to_string()),
            genre: Some("Rock".to_string()),
            track_number: Some(6),
            duration: Some(382.5),
            resource_url: "http://192.168.1.100:8201/music/track-1.flac".to_string(),
            mime_type: "audio/flac".to_string(),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_depth: Some(16),
            file_size: Some(30_000_000),
        }];

        let xml = didl_lite(&[], &items);
        assert!(xml.contains("Comfortably Numb"));
        assert!(xml.contains("Pink Floyd"));
        assert!(xml.contains("The Wall"));
        assert!(xml.contains("Rock"));
        assert!(xml.contains("musicTrack"));
        assert!(xml.contains("audio/flac"));
        assert!(xml.contains("sampleFrequency=\"44100\""));
        assert!(xml.contains("duration=\"00:06:22.500\""));
    }

    #[test]
    fn test_didl_container() {
        let containers = vec![DidlContainer {
            id: "album-1".to_string(),
            parent_id: "0".to_string(),
            title: "The Wall".to_string(),
            child_count: 26,
        }];

        let xml = didl_lite(&containers, &[]);
        assert!(xml.contains("The Wall"));
        assert!(xml.contains("childCount=\"26\""));
        assert!(xml.contains("musicAlbum"));
    }

    #[test]
    fn test_xml_escaping_in_didl() {
        let items = vec![DidlItem {
            id: "1".to_string(),
            parent_id: "0".to_string(),
            title: "Rock & Roll".to_string(),
            artist: Some("AC/DC".to_string()),
            album: None,
            genre: None,
            track_number: None,
            duration: None,
            resource_url: "http://example.com/song.mp3".to_string(),
            mime_type: "audio/mpeg".to_string(),
            sample_rate: None,
            channels: None,
            bit_depth: None,
            file_size: None,
        }];

        let xml = didl_lite(&[], &items);
        assert!(xml.contains("Rock &amp; Roll"));
    }
}
