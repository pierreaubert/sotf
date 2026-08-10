#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataEditorScope {
    Album,
    Track,
}

#[derive(Debug, Clone, Default)]
pub struct MetadataEditorFields {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub year: String,
    pub genre: String,
    pub composer: String,
    pub disc_number: String,
    pub track_number: String,
    pub conductor: String,
    pub performer: String,
    pub isrc: String,
    pub ensemble: String,
    pub edition: String,
}

#[derive(Debug, Clone)]
pub struct MetadataEditorState {
    pub scope: MetadataEditorScope,
    pub target: sotf_audio_player::MetadataTarget,
    pub target_label: String,
    pub fields: MetadataEditorFields,
    pub selected_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub preview: Option<sotf_audio_player::MetadataEditPreview>,
    pub error: Option<String>,
    pub search_query: String,
    pub search_results: Vec<sotf_audio_player::MetadataImportCandidate>,
    pub selected_result: usize,
    pub search_error: Option<String>,
}

impl MetadataEditorState {
    pub const FIELD_COUNT: usize = 13;

    pub fn for_album(album: &sotf_audio_player::Album) -> Result<Self, String> {
        let album_id = album
            .id
            .ok_or_else(|| "Metadata editing requires a persisted album".to_string())?;
        let first = album.tracks.first();
        let artist = album.artist();
        let title = album.title.clone();
        Ok(Self {
            scope: MetadataEditorScope::Album,
            target: sotf_audio_player::MetadataTarget::AlbumId(album_id),
            target_label: format!("Album \"{}\"", album.title),
            fields: MetadataEditorFields {
                title: title.clone(),
                artist: artist.clone(),
                album_artist: first
                    .and_then(|track| track.album_artist.clone())
                    .unwrap_or_else(|| artist.clone()),
                year: album.year.map(|year| year.to_string()).unwrap_or_default(),
                genre: first
                    .and_then(|track| track.genre.clone())
                    .unwrap_or_default(),
                composer: first
                    .and_then(|track| track.composer.clone())
                    .unwrap_or_default(),
                disc_number: first
                    .and_then(|track| track.disc_number)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                conductor: first
                    .and_then(|track| track.conductor.clone())
                    .unwrap_or_default(),
                performer: first
                    .and_then(|track| track.performer.clone())
                    .unwrap_or_default(),
                isrc: first
                    .and_then(|track| track.isrc.clone())
                    .unwrap_or_default(),
                ensemble: first
                    .and_then(|track| track.ensemble.clone())
                    .unwrap_or_default(),
                edition: album.edition.clone().unwrap_or_default(),
                ..Default::default()
            },
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            preview: None,
            error: None,
            search_query: format!("{} {}", album.artist(), title).trim().to_string(),
            search_results: Vec::new(),
            selected_result: 0,
            search_error: None,
        })
    }

    pub fn for_track(track: &sotf_audio_player::Track) -> Self {
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| track.path.display().to_string());
        let artist = track.artist.clone().unwrap_or_default();
        Self {
            scope: MetadataEditorScope::Track,
            target: sotf_audio_player::MetadataTarget::TrackPath(track.path.clone()),
            target_label: format!("Track \"{}\"", title),
            fields: MetadataEditorFields {
                title: title.clone(),
                artist: artist.clone(),
                album_artist: track.album_artist.clone().unwrap_or_default(),
                genre: track.genre.clone().unwrap_or_default(),
                composer: track.composer.clone().unwrap_or_default(),
                disc_number: track
                    .disc_number
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                track_number: track
                    .track_number
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                conductor: track.conductor.clone().unwrap_or_default(),
                performer: track.performer.clone().unwrap_or_default(),
                isrc: track.isrc.clone().unwrap_or_default(),
                ensemble: track.ensemble.clone().unwrap_or_default(),
                edition: track.edition.clone().unwrap_or_default(),
                ..Default::default()
            },
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            preview: None,
            error: None,
            search_query: format!("{} {}", artist, title).trim().to_string(),
            search_results: Vec::new(),
            selected_result: 0,
            search_error: None,
        }
    }

    pub fn field_label(index: usize) -> &'static str {
        match index {
            0 => "Title",
            1 => "Artist",
            2 => "Album Artist",
            3 => "Year",
            4 => "Genre",
            5 => "Composer",
            6 => "Disc",
            7 => "Track",
            8 => "Conductor",
            9 => "Performer",
            10 => "ISRC",
            11 => "Ensemble",
            12 => "Edition",
            _ => "",
        }
    }

    pub fn field_value(&self, index: usize) -> &str {
        match index {
            0 => &self.fields.title,
            1 => &self.fields.artist,
            2 => &self.fields.album_artist,
            3 => &self.fields.year,
            4 => &self.fields.genre,
            5 => &self.fields.composer,
            6 => &self.fields.disc_number,
            7 => &self.fields.track_number,
            8 => &self.fields.conductor,
            9 => &self.fields.performer,
            10 => &self.fields.isrc,
            11 => &self.fields.ensemble,
            12 => &self.fields.edition,
            _ => "",
        }
    }

    pub fn set_field_value(&mut self, index: usize, value: String) {
        match index {
            0 => self.fields.title = value,
            1 => self.fields.artist = value,
            2 => self.fields.album_artist = value,
            3 => self.fields.year = value,
            4 => self.fields.genre = value,
            5 => self.fields.composer = value,
            6 => self.fields.disc_number = value,
            7 => self.fields.track_number = value,
            8 => self.fields.conductor = value,
            9 => self.fields.performer = value,
            10 => self.fields.isrc = value,
            11 => self.fields.ensemble = value,
            12 => self.fields.edition = value,
            _ => {}
        }
        self.preview = None;
        self.error = None;
    }

    pub fn patch(&self) -> Result<sotf_audio_player::MetadataPatch, String> {
        fn text(value: &str) -> Option<String> {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        fn number(label: &str, value: &str) -> Result<Option<u32>, String> {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|_| format!("{label} must be a positive number"))
        }

        Ok(sotf_audio_player::MetadataPatch {
            title: (self.scope == MetadataEditorScope::Track)
                .then(|| text(&self.fields.title))
                .flatten(),
            album_title: (self.scope == MetadataEditorScope::Album)
                .then(|| text(&self.fields.title))
                .flatten(),
            artist: text(&self.fields.artist),
            album_artist: text(&self.fields.album_artist),
            year: number("Year", &self.fields.year)?,
            genre: text(&self.fields.genre),
            composer: text(&self.fields.composer),
            disc_number: number("Disc", &self.fields.disc_number)?,
            track_number: number("Track", &self.fields.track_number)?,
            conductor: text(&self.fields.conductor),
            performer: text(&self.fields.performer),
            isrc: text(&self.fields.isrc),
            ensemble: text(&self.fields.ensemble),
            edition: text(&self.fields.edition),
        })
    }

    pub fn apply_candidate(&mut self, candidate: sotf_audio_player::MetadataImportCandidate) {
        let title = match self.scope {
            MetadataEditorScope::Album => candidate.preferred_album_title(),
            MetadataEditorScope::Track => candidate.preferred_track_title(),
        }
        .map(str::to_owned);
        if let Some(title) = title {
            self.fields.title = title;
        }
        if let Some(artist) = candidate.artist {
            self.fields.artist = artist;
        }
        if let Some(album_artist) = candidate.album_artist {
            self.fields.album_artist = album_artist;
        }
        if let Some(year) = candidate.year {
            self.fields.year = year.to_string();
        }
        if let Some(track_number) = candidate.track_number {
            self.fields.track_number = track_number.to_string();
        }
        if let Some(disc_number) = candidate.disc_number {
            self.fields.disc_number = disc_number.to_string();
        }
        if let Some(isrc) = candidate.isrc {
            self.fields.isrc = isrc;
        }
        self.preview = None;
        self.error = None;
    }
}
