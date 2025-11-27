//! Internationalization (i18n) system for the GPUI audio player.
//!
//! Provides translations for multiple languages.

use serde::{Deserialize, Serialize};

/// Available language identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    English,
    French,
    German,
    Spanish,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::French, Language::German, Language::Spanish]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::German => "de",
            Language::Spanish => "es",
        }
    }

    pub fn next(&self) -> Language {
        match self {
            Language::English => Language::French,
            Language::French => Language::German,
            Language::German => Language::Spanish,
            Language::Spanish => Language::English,
        }
    }
}

/// All translatable strings in the application
#[derive(Debug, Clone)]
pub struct Translations {
    // App title
    pub app_title: &'static str,

    // Screen names
    pub screen_library: &'static str,
    pub screen_directories: &'static str,
    pub screen_queue: &'static str,
    pub screen_spectrum: &'static str,
    pub screen_plugins: &'static str,
    pub screen_devices: &'static str,
    pub screen_settings: &'static str,

    // Library screen
    pub library_title: &'static str,
    pub library_albums: &'static str,
    pub library_tracks: &'static str,
    pub library_search_placeholder: &'static str,
    pub library_search_hint: &'static str,
    pub library_sort: &'static str,
    pub library_filter: &'static str,
    pub library_view_flat: &'static str,
    pub library_view_tree: &'static str,
    pub library_scan: &'static str,
    pub library_scanning: &'static str,
    pub library_page: &'static str,
    pub library_of: &'static str,
    pub library_items_per_page: &'static str,
    pub library_prev: &'static str,
    pub library_next: &'static str,

    // Sort options
    pub sort_artist: &'static str,
    pub sort_album: &'static str,
    pub sort_title: &'static str,
    pub sort_year: &'static str,

    // Filter options
    pub filter_all: &'static str,
    pub filter_mono: &'static str,
    pub filter_stereo: &'static str,
    pub filter_multichannel: &'static str,
    pub filter_mixed: &'static str,

    // Queue screen
    pub queue_title: &'static str,
    pub queue_clear: &'static str,
    pub queue_track: &'static str,
    pub queue_empty: &'static str,

    // Level meters
    pub level_meters_title: &'static str,
    pub level_meters_no_audio: &'static str,
    pub level_meters_hint: &'static str,

    // Devices screen
    pub devices_title: &'static str,
    pub devices_default: &'static str,

    // Spectrum screen
    pub spectrum_title: &'static str,
    pub spectrum_no_data: &'static str,

    // Directory screen
    pub directories_title: &'static str,
    pub directories_add: &'static str,
    pub directories_hint: &'static str,
    pub directories_scan_hint: &'static str,

    // Plugins screen
    pub plugins_title: &'static str,
    pub plugins_chain: &'static str,
    pub plugins_add: &'static str,
    pub plugins_enabled: &'static str,
    pub plugins_disabled: &'static str,

    // Settings screen
    pub settings_title: &'static str,
    pub settings_theme: &'static str,
    pub settings_language: &'static str,

    // Playback controls
    pub playback_play: &'static str,
    pub playback_pause: &'static str,
    pub playback_stop: &'static str,
    pub playback_next: &'static str,
    pub playback_previous: &'static str,
    pub playback_volume: &'static str,

    // Dialog titles
    pub dialog_help: &'static str,
    pub dialog_load_apo: &'static str,
    pub dialog_load_sofa: &'static str,
    pub dialog_save_preset: &'static str,
    pub dialog_load_preset: &'static str,
    pub dialog_edit_plugin: &'static str,

    // Dialog content
    pub dialog_enter_path: &'static str,
    pub dialog_enter_name: &'static str,
    pub dialog_existing_presets: &'static str,
    pub dialog_available_presets: &'static str,
    pub dialog_no_presets: &'static str,

    // Button labels
    pub button_save: &'static str,
    pub button_load: &'static str,
    pub button_cancel: &'static str,
    pub button_close: &'static str,
    pub button_apply: &'static str,
    pub button_ok: &'static str,

    // Keyboard hints
    pub key_enter: &'static str,
    pub key_escape: &'static str,
    pub key_tab: &'static str,
    pub key_space: &'static str,
    pub key_arrows: &'static str,

    // Status messages
    pub status_scan_complete: &'static str,
    pub status_scan_failed: &'static str,
    pub status_preset_saved: &'static str,
    pub status_preset_loaded: &'static str,
    pub status_directory_added: &'static str,
    pub status_directory_removed: &'static str,

    // Global keybindings
    pub keybind_global: &'static str,
    pub keybind_play_pause: &'static str,
    pub keybind_next_track: &'static str,
    pub keybind_volume: &'static str,

    // Mute/Solo/Dim
    pub control_mute: &'static str,
    pub control_solo: &'static str,
    pub control_dim: &'static str,
    pub control_clear_all: &'static str,
}

impl Translations {
    /// Get translations for a specific language
    pub fn for_language(language: Language) -> Self {
        match language {
            Language::English => Self::english(),
            Language::French => Self::french(),
            Language::German => Self::german(),
            Language::Spanish => Self::spanish(),
        }
    }

    /// English translations (default)
    pub fn english() -> Self {
        Self {
            app_title: "SOTF Audio Player",

            screen_library: "Library",
            screen_directories: "Directories",
            screen_queue: "Queue",
            screen_spectrum: "Spectrum",
            screen_plugins: "Plugins",
            screen_devices: "Devices",
            screen_settings: "Settings",

            library_title: "Library",
            library_albums: "albums",
            library_tracks: "tracks",
            library_search_placeholder: "Type to search...",
            library_search_hint: "Press / to search",
            library_sort: "Sort",
            library_filter: "Filter",
            library_view_flat: "Flat",
            library_view_tree: "Tree",
            library_scan: "Scan",
            library_scanning: "Scanning...",
            library_page: "Page",
            library_of: "of",
            library_items_per_page: "items/page",
            library_prev: "← Prev",
            library_next: "Next →",

            sort_artist: "Artist",
            sort_album: "Album",
            sort_title: "Title",
            sort_year: "Year",

            filter_all: "All",
            filter_mono: "Mono",
            filter_stereo: "Stereo",
            filter_multichannel: "Multi",
            filter_mixed: "Mixed",

            queue_title: "Queue",
            queue_clear: "Clear",
            queue_track: "Track",
            queue_empty: "Queue is empty",

            level_meters_title: "Level Meters",
            level_meters_no_audio: "No audio playing",
            level_meters_hint: "Tab: Select group | M: Mute | Shift-M: Solo | Ctrl-M: Dim | X: Clear",

            devices_title: "Audio Output Devices",
            devices_default: "Default",

            spectrum_title: "Spectrum Analyzer",
            spectrum_no_data: "No spectrum data available. Play audio to see visualization.",

            directories_title: "Directory Manager",
            directories_add: "Add Directory",
            directories_hint: "Tab: autocomplete, Enter: add, Esc: cancel",
            directories_scan_hint: "Shift-A: Add | Shift-S: Scan | D: Remove | Enter: Expand",

            plugins_title: "Plugin Chain",
            plugins_chain: "Plugins",
            plugins_add: "Add Plugin",
            plugins_enabled: "Enabled",
            plugins_disabled: "Disabled",

            settings_title: "Settings",
            settings_theme: "Theme",
            settings_language: "Language",

            playback_play: "Play",
            playback_pause: "Pause",
            playback_stop: "Stop",
            playback_next: "Next",
            playback_previous: "Previous",
            playback_volume: "Volume",

            dialog_help: "Help",
            dialog_load_apo: "Load APO File for EQ Plugin",
            dialog_load_sofa: "Load SOFA File for Binaural Decoder",
            dialog_save_preset: "Save Plugin Preset",
            dialog_load_preset: "Load Plugin Preset",
            dialog_edit_plugin: "Edit Plugin",

            dialog_enter_path: "Enter path:",
            dialog_enter_name: "Enter preset name:",
            dialog_existing_presets: "Existing presets:",
            dialog_available_presets: "Available presets:",
            dialog_no_presets: "No presets found. Save a preset first.",

            button_save: "Save",
            button_load: "Load",
            button_cancel: "Cancel",
            button_close: "Close",
            button_apply: "Apply",
            button_ok: "OK",

            key_enter: "Enter",
            key_escape: "ESC",
            key_tab: "Tab",
            key_space: "Space",
            key_arrows: "↑/↓",

            status_scan_complete: "Scan complete",
            status_scan_failed: "Scan failed",
            status_preset_saved: "Preset saved",
            status_preset_loaded: "Preset loaded",
            status_directory_added: "Directory added",
            status_directory_removed: "Directory removed",

            keybind_global: "GLOBAL KEYBINDINGS",
            keybind_play_pause: "Space: Play/Pause",
            keybind_next_track: "N: Next",
            keybind_volume: "+/-: Volume",

            control_mute: "M",
            control_solo: "S",
            control_dim: "D",
            control_clear_all: "Clear all",
        }
    }

    /// French translations
    pub fn french() -> Self {
        Self {
            app_title: "SOTF Lecteur Audio",

            screen_library: "Bibliothèque",
            screen_directories: "Répertoires",
            screen_queue: "File d'attente",
            screen_spectrum: "Spectre",
            screen_plugins: "Plugins",
            screen_devices: "Périphériques",
            screen_settings: "Paramètres",

            library_title: "Bibliothèque",
            library_albums: "albums",
            library_tracks: "pistes",
            library_search_placeholder: "Rechercher...",
            library_search_hint: "Appuyez / pour rechercher",
            library_sort: "Tri",
            library_filter: "Filtre",
            library_view_flat: "Liste",
            library_view_tree: "Arbre",
            library_scan: "Scanner",
            library_scanning: "Scan en cours...",
            library_page: "Page",
            library_of: "sur",
            library_items_per_page: "éléments/page",
            library_prev: "← Préc",
            library_next: "Suiv →",

            sort_artist: "Artiste",
            sort_album: "Album",
            sort_title: "Titre",
            sort_year: "Année",

            filter_all: "Tous",
            filter_mono: "Mono",
            filter_stereo: "Stéréo",
            filter_multichannel: "Multi",
            filter_mixed: "Mixte",

            queue_title: "File d'attente",
            queue_clear: "Vider",
            queue_track: "Piste",
            queue_empty: "File d'attente vide",

            level_meters_title: "Niveaux",
            level_meters_no_audio: "Aucun audio en lecture",
            level_meters_hint: "Tab: Groupe | M: Mute | Shift-M: Solo | Ctrl-M: Dim | X: Effacer",

            devices_title: "Périphériques de sortie audio",
            devices_default: "Par défaut",

            spectrum_title: "Analyseur de spectre",
            spectrum_no_data: "Aucune donnée spectrale. Lancez la lecture pour voir la visualisation.",

            directories_title: "Gestionnaire de répertoires",
            directories_add: "Ajouter un répertoire",
            directories_hint: "Tab: compléter, Entrée: ajouter, Échap: annuler",
            directories_scan_hint: "Shift-A: Ajouter | Shift-S: Scanner | D: Supprimer | Entrée: Développer",

            plugins_title: "Chaîne de plugins",
            plugins_chain: "Plugins",
            plugins_add: "Ajouter un plugin",
            plugins_enabled: "Activé",
            plugins_disabled: "Désactivé",

            settings_title: "Paramètres",
            settings_theme: "Thème",
            settings_language: "Langue",

            playback_play: "Lecture",
            playback_pause: "Pause",
            playback_stop: "Stop",
            playback_next: "Suivant",
            playback_previous: "Précédent",
            playback_volume: "Volume",

            dialog_help: "Aide",
            dialog_load_apo: "Charger un fichier APO pour l'EQ",
            dialog_load_sofa: "Charger un fichier SOFA pour le décodeur binaural",
            dialog_save_preset: "Sauvegarder le preset",
            dialog_load_preset: "Charger un preset",
            dialog_edit_plugin: "Modifier le plugin",

            dialog_enter_path: "Entrez le chemin:",
            dialog_enter_name: "Nom du preset:",
            dialog_existing_presets: "Presets existants:",
            dialog_available_presets: "Presets disponibles:",
            dialog_no_presets: "Aucun preset trouvé. Sauvegardez d'abord un preset.",

            button_save: "Sauver",
            button_load: "Charger",
            button_cancel: "Annuler",
            button_close: "Fermer",
            button_apply: "Appliquer",
            button_ok: "OK",

            key_enter: "Entrée",
            key_escape: "Échap",
            key_tab: "Tab",
            key_space: "Espace",
            key_arrows: "↑/↓",

            status_scan_complete: "Scan terminé",
            status_scan_failed: "Échec du scan",
            status_preset_saved: "Preset sauvegardé",
            status_preset_loaded: "Preset chargé",
            status_directory_added: "Répertoire ajouté",
            status_directory_removed: "Répertoire supprimé",

            keybind_global: "RACCOURCIS GLOBAUX",
            keybind_play_pause: "Espace: Lecture/Pause",
            keybind_next_track: "N: Suivant",
            keybind_volume: "+/-: Volume",

            control_mute: "M",
            control_solo: "S",
            control_dim: "D",
            control_clear_all: "Tout effacer",
        }
    }

    /// German translations
    pub fn german() -> Self {
        Self {
            app_title: "SOTF Audioplayer",

            screen_library: "Bibliothek",
            screen_directories: "Verzeichnisse",
            screen_queue: "Warteschlange",
            screen_spectrum: "Spektrum",
            screen_plugins: "Plugins",
            screen_devices: "Geräte",
            screen_settings: "Einstellungen",

            library_title: "Bibliothek",
            library_albums: "Alben",
            library_tracks: "Titel",
            library_search_placeholder: "Suchen...",
            library_search_hint: "Drücken Sie / zum Suchen",
            library_sort: "Sortierung",
            library_filter: "Filter",
            library_view_flat: "Liste",
            library_view_tree: "Baum",
            library_scan: "Scannen",
            library_scanning: "Scanne...",
            library_page: "Seite",
            library_of: "von",
            library_items_per_page: "Einträge/Seite",
            library_prev: "← Zurück",
            library_next: "Weiter →",

            sort_artist: "Künstler",
            sort_album: "Album",
            sort_title: "Titel",
            sort_year: "Jahr",

            filter_all: "Alle",
            filter_mono: "Mono",
            filter_stereo: "Stereo",
            filter_multichannel: "Multi",
            filter_mixed: "Gemischt",

            queue_title: "Warteschlange",
            queue_clear: "Leeren",
            queue_track: "Titel",
            queue_empty: "Warteschlange ist leer",

            level_meters_title: "Pegelanzeige",
            level_meters_no_audio: "Keine Audiowiedergabe",
            level_meters_hint: "Tab: Gruppe | M: Stumm | Shift-M: Solo | Ctrl-M: Dim | X: Löschen",

            devices_title: "Audioausgabegeräte",
            devices_default: "Standard",

            spectrum_title: "Spektrumanalysator",
            spectrum_no_data: "Keine Spektrumdaten verfügbar. Starten Sie die Wiedergabe.",

            directories_title: "Verzeichnisverwaltung",
            directories_add: "Verzeichnis hinzufügen",
            directories_hint: "Tab: Vervollständigen, Enter: Hinzufügen, Esc: Abbrechen",
            directories_scan_hint: "Shift-A: Hinzufügen | Shift-S: Scannen | D: Entfernen | Enter: Erweitern",

            plugins_title: "Plugin-Kette",
            plugins_chain: "Plugins",
            plugins_add: "Plugin hinzufügen",
            plugins_enabled: "Aktiviert",
            plugins_disabled: "Deaktiviert",

            settings_title: "Einstellungen",
            settings_theme: "Design",
            settings_language: "Sprache",

            playback_play: "Wiedergabe",
            playback_pause: "Pause",
            playback_stop: "Stopp",
            playback_next: "Nächster",
            playback_previous: "Vorheriger",
            playback_volume: "Lautstärke",

            dialog_help: "Hilfe",
            dialog_load_apo: "APO-Datei für EQ laden",
            dialog_load_sofa: "SOFA-Datei für Binaural-Decoder laden",
            dialog_save_preset: "Preset speichern",
            dialog_load_preset: "Preset laden",
            dialog_edit_plugin: "Plugin bearbeiten",

            dialog_enter_path: "Pfad eingeben:",
            dialog_enter_name: "Preset-Name:",
            dialog_existing_presets: "Vorhandene Presets:",
            dialog_available_presets: "Verfügbare Presets:",
            dialog_no_presets: "Keine Presets gefunden. Speichern Sie zuerst ein Preset.",

            button_save: "Speichern",
            button_load: "Laden",
            button_cancel: "Abbrechen",
            button_close: "Schließen",
            button_apply: "Anwenden",
            button_ok: "OK",

            key_enter: "Enter",
            key_escape: "Esc",
            key_tab: "Tab",
            key_space: "Leertaste",
            key_arrows: "↑/↓",

            status_scan_complete: "Scan abgeschlossen",
            status_scan_failed: "Scan fehlgeschlagen",
            status_preset_saved: "Preset gespeichert",
            status_preset_loaded: "Preset geladen",
            status_directory_added: "Verzeichnis hinzugefügt",
            status_directory_removed: "Verzeichnis entfernt",

            keybind_global: "GLOBALE TASTENKÜRZEL",
            keybind_play_pause: "Leertaste: Wiedergabe/Pause",
            keybind_next_track: "N: Nächster",
            keybind_volume: "+/-: Lautstärke",

            control_mute: "M",
            control_solo: "S",
            control_dim: "D",
            control_clear_all: "Alle löschen",
        }
    }

    /// Spanish translations
    pub fn spanish() -> Self {
        Self {
            app_title: "SOTF Reproductor de Audio",

            screen_library: "Biblioteca",
            screen_directories: "Directorios",
            screen_queue: "Cola",
            screen_spectrum: "Espectro",
            screen_plugins: "Plugins",
            screen_devices: "Dispositivos",
            screen_settings: "Ajustes",

            library_title: "Biblioteca",
            library_albums: "álbumes",
            library_tracks: "pistas",
            library_search_placeholder: "Buscar...",
            library_search_hint: "Presiona / para buscar",
            library_sort: "Ordenar",
            library_filter: "Filtro",
            library_view_flat: "Lista",
            library_view_tree: "Árbol",
            library_scan: "Escanear",
            library_scanning: "Escaneando...",
            library_page: "Página",
            library_of: "de",
            library_items_per_page: "elementos/página",
            library_prev: "← Anterior",
            library_next: "Siguiente →",

            sort_artist: "Artista",
            sort_album: "Álbum",
            sort_title: "Título",
            sort_year: "Año",

            filter_all: "Todos",
            filter_mono: "Mono",
            filter_stereo: "Estéreo",
            filter_multichannel: "Multi",
            filter_mixed: "Mixto",

            queue_title: "Cola",
            queue_clear: "Limpiar",
            queue_track: "Pista",
            queue_empty: "Cola vacía",

            level_meters_title: "Medidores de nivel",
            level_meters_no_audio: "Sin audio reproduciéndose",
            level_meters_hint: "Tab: Grupo | M: Silenciar | Shift-M: Solo | Ctrl-M: Dim | X: Limpiar",

            devices_title: "Dispositivos de salida de audio",
            devices_default: "Predeterminado",

            spectrum_title: "Analizador de espectro",
            spectrum_no_data: "Sin datos de espectro. Reproduce audio para ver la visualización.",

            directories_title: "Gestor de directorios",
            directories_add: "Añadir directorio",
            directories_hint: "Tab: completar, Enter: añadir, Esc: cancelar",
            directories_scan_hint: "Shift-A: Añadir | Shift-S: Escanear | D: Eliminar | Enter: Expandir",

            plugins_title: "Cadena de plugins",
            plugins_chain: "Plugins",
            plugins_add: "Añadir plugin",
            plugins_enabled: "Activado",
            plugins_disabled: "Desactivado",

            settings_title: "Ajustes",
            settings_theme: "Tema",
            settings_language: "Idioma",

            playback_play: "Reproducir",
            playback_pause: "Pausa",
            playback_stop: "Detener",
            playback_next: "Siguiente",
            playback_previous: "Anterior",
            playback_volume: "Volumen",

            dialog_help: "Ayuda",
            dialog_load_apo: "Cargar archivo APO para EQ",
            dialog_load_sofa: "Cargar archivo SOFA para decodificador binaural",
            dialog_save_preset: "Guardar preset",
            dialog_load_preset: "Cargar preset",
            dialog_edit_plugin: "Editar plugin",

            dialog_enter_path: "Ingrese la ruta:",
            dialog_enter_name: "Nombre del preset:",
            dialog_existing_presets: "Presets existentes:",
            dialog_available_presets: "Presets disponibles:",
            dialog_no_presets: "No se encontraron presets. Guarde un preset primero.",

            button_save: "Guardar",
            button_load: "Cargar",
            button_cancel: "Cancelar",
            button_close: "Cerrar",
            button_apply: "Aplicar",
            button_ok: "OK",

            key_enter: "Enter",
            key_escape: "Esc",
            key_tab: "Tab",
            key_space: "Espacio",
            key_arrows: "↑/↓",

            status_scan_complete: "Escaneo completado",
            status_scan_failed: "Escaneo fallido",
            status_preset_saved: "Preset guardado",
            status_preset_loaded: "Preset cargado",
            status_directory_added: "Directorio añadido",
            status_directory_removed: "Directorio eliminado",

            keybind_global: "ATAJOS GLOBALES",
            keybind_play_pause: "Espacio: Reproducir/Pausa",
            keybind_next_track: "N: Siguiente",
            keybind_volume: "+/-: Volumen",

            control_mute: "M",
            control_solo: "S",
            control_dim: "D",
            control_clear_all: "Limpiar todo",
        }
    }
}
