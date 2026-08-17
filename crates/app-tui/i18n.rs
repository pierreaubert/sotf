use crate::app::Screen;

/// Localize first-party terminal copy at a render boundary.
#[macro_export]
macro_rules! tui_text {
    ($app:expr, $message:expr) => {
        $crate::i18n::TuiTranslations::for_language($app.ui.language)
            .dynamic(($message).to_string())
    };
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    French,
    German,
    Spanish,
}

impl Language {
    pub const ALL: [Self; 4] = [Self::English, Self::French, Self::German, Self::Spanish];

    pub fn from_environment() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .map_or(Self::English, |locale| Self::from_locale(&locale))
    }

    pub fn from_locale(locale: &str) -> Self {
        match locale
            .split(['.', '_', '-'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "fr" => Self::French,
            "de" => Self::German,
            "es" => Self::Spanish,
            _ => Self::English,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::English => Self::French,
            Self::French => Self::German,
            Self::German => Self::Spanish,
            Self::Spanish => Self::English,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
            Self::German => "de",
            Self::Spanish => "es",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TuiTranslations {
    language: Language,
    pub help: &'static str,
    pub global_keybindings: &'static str,
    pub level_meters_focused: &'static str,
    pub level_meters_global: &'static str,
    pub cycle_language: &'static str,
    pub language_changed: &'static str,
}

impl TuiTranslations {
    pub fn for_language(language: Language) -> Self {
        match language {
            Language::English => Self {
                language,
                help: "Help",
                global_keybindings: "GLOBAL KEYBINDINGS",
                level_meters_focused: "LEVEL METERS (when the Meters pane is focused)",
                level_meters_global: "LEVEL METERS (global shortcuts)",
                cycle_language: "Cycle language",
                language_changed: "Language: English",
            },
            Language::French => Self {
                language,
                help: "Aide",
                global_keybindings: "RACCOURCIS GLOBAUX",
                level_meters_focused: "INDICATEURS DE NIVEAU (volet Indicateurs actif)",
                level_meters_global: "INDICATEURS DE NIVEAU (raccourcis globaux)",
                cycle_language: "Changer de langue",
                language_changed: "Langue : français",
            },
            Language::German => Self {
                language,
                help: "Hilfe",
                global_keybindings: "GLOBALE TASTENKÜRZEL",
                level_meters_focused: "PEGELANZEIGEN (Pegelbereich fokussiert)",
                level_meters_global: "PEGELANZEIGEN (globale Tastenkürzel)",
                cycle_language: "Sprache wechseln",
                language_changed: "Sprache: Deutsch",
            },
            Language::Spanish => Self {
                language,
                help: "Ayuda",
                global_keybindings: "ATAJOS GLOBALES",
                level_meters_focused: "MEDIDORES DE NIVEL (panel Medidores activo)",
                level_meters_global: "MEDIDORES DE NIVEL (atajos globales)",
                cycle_language: "Cambiar el idioma",
                language_changed: "Idioma: español",
            },
        }
    }

    pub fn screen_name(self, screen: Screen) -> &'static str {
        match (self.language, screen) {
            (Language::English, Screen::Loading) => "Loading",
            (Language::English, Screen::Library) => "Library",
            (Language::English, Screen::Queue) => "Queue",
            (Language::English, Screen::Playlists) => "Playlists",
            (Language::English, Screen::Plugins) => "Plugins",
            (Language::English, Screen::Devices) => "Devices",
            (Language::English, Screen::Tools) => "Tools",
            (Language::English, Screen::EarTraining) => "Ear Training",
            (Language::English, Screen::AbTesting) => "A/B Testing",
            (Language::English, Screen::Configure) => "Configure",
            (Language::French, Screen::Loading) => "Chargement",
            (Language::French, Screen::Library) => "Bibliothèque",
            (Language::French, Screen::Queue) => "File d’attente",
            (Language::French, Screen::Playlists) => "Listes de lecture",
            (Language::French, Screen::Plugins) => "Modules",
            (Language::French, Screen::Devices) => "Périphériques",
            (Language::French, Screen::Tools) => "Outils",
            (Language::French, Screen::EarTraining) => "Entraînement auditif",
            (Language::French, Screen::AbTesting) => "Test A/B",
            (Language::French, Screen::Configure) => "Configuration",
            (Language::German, Screen::Loading) => "Laden",
            (Language::German, Screen::Library) => "Mediathek",
            (Language::German, Screen::Queue) => "Warteschlange",
            (Language::German, Screen::Playlists) => "Wiedergabelisten",
            (Language::German, Screen::Plugins) => "Plugins",
            (Language::German, Screen::Devices) => "Geräte",
            (Language::German, Screen::Tools) => "Werkzeuge",
            (Language::German, Screen::EarTraining) => "Gehörtraining",
            (Language::German, Screen::AbTesting) => "A/B-Test",
            (Language::German, Screen::Configure) => "Konfiguration",
            (Language::Spanish, Screen::Loading) => "Cargando",
            (Language::Spanish, Screen::Library) => "Biblioteca",
            (Language::Spanish, Screen::Queue) => "Cola",
            (Language::Spanish, Screen::Playlists) => "Listas",
            (Language::Spanish, Screen::Plugins) => "Complementos",
            (Language::Spanish, Screen::Devices) => "Dispositivos",
            (Language::Spanish, Screen::Tools) => "Herramientas",
            (Language::Spanish, Screen::EarTraining) => "Entrenamiento auditivo",
            (Language::Spanish, Screen::AbTesting) => "Prueba A/B",
            (Language::Spanish, Screen::Configure) => "Configuración",
        }
    }

    pub fn help_title(self, screen: Screen) -> String {
        match self.language {
            Language::English => format!(
                "{} - {} screen (press ESC or ? to close)",
                self.help,
                self.screen_name(screen)
            ),
            Language::French => format!(
                "{} — écran {} (ESC ou ? pour fermer)",
                self.help,
                self.screen_name(screen)
            ),
            Language::German => format!(
                "{} – {} (mit ESC oder ? schließen)",
                self.help,
                self.screen_name(screen)
            ),
            Language::Spanish => format!(
                "{} — pantalla {} (ESC o ? para cerrar)",
                self.help,
                self.screen_name(screen)
            ),
        }
    }

    pub fn action_description(self, action: &'static str) -> &'static str {
        let translations = match self.language {
            Language::English => return action,
            Language::French => FRENCH_ACTIONS,
            Language::German => GERMAN_ACTIONS,
            Language::Spanish => SPANISH_ACTIONS,
        };

        translations
            .iter()
            .find_map(|(source, translation)| (*source == action).then_some(*translation))
            .unwrap_or_else(|| panic!("missing localized TUI action: {action}"))
    }

    /// Translate first-party static copy used by terminal screens.
    ///
    /// External plugin-provided labels intentionally bypass this catalog and
    /// remain verbatim. Every call site using this method is parity-tested
    /// across the required locale set.
    pub fn ui(self, source: &'static str) -> &'static str {
        let translations = match self.language {
            Language::English => return source,
            Language::French => FRENCH_UI,
            Language::German => GERMAN_UI,
            Language::Spanish => SPANISH_UI,
        };

        translations
            .iter()
            .find_map(|(key, translation)| (*key == source).then_some(*translation))
            .unwrap_or_else(|| panic!("missing localized TUI copy: {source}"))
    }

    /// Translate a formatted first-party status while preserving dynamic
    /// values and verbatim external error details.
    pub fn dynamic(self, message: String) -> String {
        if self.language == Language::English {
            return message;
        }

        if let Some(localized) = self.try_dynamic(&message) {
            return localized;
        }

        panic!("missing localized TUI dynamic copy: {message}")
    }

    /// Translate known first-party framing while retaining unknown external
    /// diagnostics verbatim. Use this only at boundaries that can carry both.
    pub fn dynamic_or_verbatim(self, message: &str) -> String {
        if self.language == Language::English {
            return message.to_string();
        }
        self.try_dynamic(message)
            .unwrap_or_else(|| message.to_string())
    }

    fn try_dynamic(self, message: &str) -> Option<String> {
        for translation in DYNAMIC_TRANSLATIONS {
            let target = match self.language {
                Language::English => translation.english,
                Language::French => translation.french,
                Language::German => translation.german,
                Language::Spanish => translation.spanish,
            };
            if let Some(localized) = translate_pattern(message, translation.english, target) {
                return Some(localized);
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
struct DynamicTranslation {
    english: &'static str,
    french: &'static str,
    german: &'static str,
    spanish: &'static str,
}

const fn dynamic_translation(
    english: &'static str,
    french: &'static str,
    german: &'static str,
    spanish: &'static str,
) -> DynamicTranslation {
    DynamicTranslation {
        english,
        french,
        german,
        spanish,
    }
}

fn translate_pattern(message: &str, source: &str, target: &str) -> Option<String> {
    let source_parts: Vec<&str> = source.split("{}").collect();
    if source_parts.len() == 1 {
        return (message == source).then(|| target.to_string());
    }

    let target_parts: Vec<&str> = target.split("{}").collect();
    if target_parts.len() != source_parts.len() {
        return None;
    }

    let mut rest = message;
    let first = source_parts[0];
    if !rest.starts_with(first) {
        return None;
    }
    rest = &rest[first.len()..];

    let mut captures = Vec::with_capacity(source_parts.len() - 1);
    for (index, part) in source_parts.iter().enumerate().skip(1) {
        if index == source_parts.len() - 1 {
            if !rest.ends_with(part) {
                return None;
            }
            captures.push(&rest[..rest.len() - part.len()]);
            rest = "";
        } else {
            let boundary = rest.find(part)?;
            captures.push(&rest[..boundary]);
            rest = &rest[boundary + part.len()..];
        }
    }
    if !rest.is_empty() {
        return None;
    }

    let mut localized = String::new();
    localized.push_str(target_parts[0]);
    for (capture, part) in captures.iter().zip(target_parts.iter().skip(1)) {
        localized.push_str(capture);
        localized.push_str(part);
    }
    Some(localized)
}

const FRENCH_UI: &[(&str, &str)] = &[
    ("Loudness", "Sonie"),
    ("True Peak", "Crête vraie"),
    ("Stereo width", "Largeur stéréo"),
    ("No audio playing", "Aucun son en lecture"),
    ("No audio", "Aucun son"),
    ("Levels", "Niveaux"),
    ("No channels", "Aucun canal"),
    ("Levels (help: ?)", "Niveaux (aide : ?)"),
    (
        " 1=Simple  2=Full  3/4/5=layout  Tab=next step",
        " 1=Simple  2=Complet  3/4/5=configuration  Tab=étape suivante",
    ),
    (" = cancel", " = annuler"),
    (
        " Choose your optimization workflow",
        " Choisissez votre flux d’optimisation",
    ),
    (
        " Configure – select a workflow ",
        " Configuration – choisissez un flux ",
    ),
    (
        " Enter=edit path, type path and Enter to export",
        " Entrée=modifier le chemin, saisissez-le puis Entrée pour exporter",
    ),
    (" Export successful!", " Exportation réussie !"),
    (" Metadata Services ", " Services de métadonnées "),
    (
        " No bass-anchor results yet — optional step.",
        " Aucun résultat d’ancrage des graves — étape facultative.",
    ),
    (
        " No channels yet — record some first.",
        " Aucun canal — effectuez d’abord des enregistrements.",
    ),
    (" No data loaded", " Aucune donnée chargée"),
    (
        " No probe captured yet — press \x60r\x60 to run",
        " Aucune sonde capturée — appuyez sur \x60r\x60 pour l’exécuter",
    ),
    (
        " Per-Channel Alignment Delays",
        " Délais d’alignement par canal",
    ),
    (
        " Recordings saved successfully!",
        " Enregistrements sauvegardés !",
    ),
    (" Tab=next step", " Tab=étape suivante"),
    (
        " Up/Down=select  Enter=record  A=auto-record all  Tab=evaluate",
        " Haut/Bas=sélectionner  Entrée=enregistrer  A=tout automatiser  Tab=évaluer",
    ),
    (
        " Up/Down=select channel  Tab=save  BackTab=capture",
        " Haut/Bas=choisir le canal  Tab=sauvegarder  Maj+Tab=capturer",
    ),
    ("Align ms", "Alignement ms"),
    ("Alignment Delays", "Délais d’alignement"),
    ("Apply Error", "Erreur d’application"),
    ("Apply Status", "État de l’application"),
    ("Apply", "Appliquer"),
    ("Arrival ms", "Arrivée ms"),
    ("Bass Anchor", "Ancrage des graves"),
    ("Channel", "Canal"),
    (
        "Channels (Up/Down to select)",
        "Canaux (Haut/Bas pour choisir)",
    ),
    ("Channels", "Canaux"),
    ("Channels: ", "Canaux : "),
    ("Configuration", "Configuration"),
    ("Configure", "Configurer"),
    ("Delay Probe Capture", "Capture de la sonde de délai"),
    ("Delay ms", "Délai ms"),
    ("Delay", "Délai"),
    ("Details", "Détails"),
    ("Duration (s)", "Durée (s)"),
    ("Error", "Erreur"),
    ("Evaluate", "Évaluer"),
    ("Filters", "Filtres"),
    ("Freq (Hz)", "Fréq. (Hz)"),
    ("Gain (dB)", "Gain (dB)"),
    ("Gain dB", "Gain dB"),
    ("Headphone EQ", "Égalisation casque"),
    ("Loss History", "Historique de perte"),
    ("Loss", "Perte"),
    (
        "Manual album/track metadata edits use the shared metadata controller.",
        "Les modifications manuelles des métadonnées utilisent le contrôleur partagé.",
    ),
    ("Measurement: ", "Mesure : "),
    ("Mic input channel", "Canal d’entrée micro"),
    (
        "MusicBrainz search/import is anonymous by default; login is optional.",
        "La recherche et l’import MusicBrainz sont anonymes par défaut ; la connexion est facultative.",
    ),
    ("No log messages yet", "Aucun message de journal"),
    (
        "No optimization results yet. Go to Optimize step first.",
        "Aucun résultat d’optimisation. Passez d’abord à l’étape Optimiser.",
    ),
    (
        "No recordings completed yet. Go to Capture step.",
        "Aucun enregistrement terminé. Passez à l’étape Capture.",
    ),
    (
        "No results yet. Go to Optimize step and run optimization.",
        "Aucun résultat. Passez à Optimiser et lancez l’optimisation.",
    ),
    ("OK", "OK"),
    ("Optimization", "Optimisation"),
    ("Output channel", "Canal de sortie"),
    ("PEQ Filters", "Filtres PEQ"),
    ("Phase °", "Phase °"),
    ("Points", "Points"),
    ("Post Score", "Score après"),
    ("Pre Score", "Score avant"),
    ("Probe duration (ms)", "Durée de la sonde (ms)"),
    ("Process", "Traiter"),
    ("Progress", "Progression"),
    ("Q", "Q"),
    ("Quality", "Qualité"),
    ("Recorded Channels", "Canaux enregistrés"),
    ("Recording", "Enregistrement"),
    ("Reference freq (Hz)", "Fréq. de référence (Hz)"),
    ("Reported dBSPL", "dB SPL indiqué"),
    ("Result", "Résultat"),
    ("Results", "Résultats"),
    ("Review", "Vérifier"),
    ("Room EQ", "Correction de salle"),
    ("SNR dB", "SNR dB"),
    ("SPL Calibration", "Étalonnage SPL"),
    ("Silence gap (ms)", "Intervalle de silence (ms)"),
    (
        "Source (Left/Right to toggle)",
        "Source (Gauche/Droite pour basculer)",
    ),
    ("Speaker: ", "Enceinte : "),
    ("Spinorama EQ", "Égalisation Spinorama"),
    ("Stab °", "Stabilité °"),
    ("State", "État"),
    ("Status", "État"),
    ("Suggestions (spinorama.org)", "Suggestions (spinorama.org)"),
    ("Summary", "Résumé"),
    (
        "Target Preset (Left/Right to cycle)",
        "Cible (Gauche/Droite pour parcourir)",
    ),
    ("Tone amplitude (0-1)", "Amplitude du signal (0-1)"),
    ("Type", "Type"),
    ("Update Plugin", "Mettre à jour le module"),
    ("[ Run Probe ]", "[ Exécuter la sonde ]"),
    ("|mag|", "|mag|"),
];

const GERMAN_UI: &[(&str, &str)] = &[
    ("Loudness", "Lautheit"),
    ("True Peak", "True Peak"),
    ("Stereo width", "Stereobreite"),
    ("No audio playing", "Keine Audiowiedergabe"),
    ("No audio", "Kein Audio"),
    ("Levels", "Pegel"),
    ("No channels", "Keine Kanäle"),
    ("Levels (help: ?)", "Pegel (Hilfe: ?)"),
    (
        " 1=Simple  2=Full  3/4/5=layout  Tab=next step",
        " 1=Einfach  2=Vollständig  3/4/5=Layout  Tab=nächster Schritt",
    ),
    (" = cancel", " = abbrechen"),
    (
        " Choose your optimization workflow",
        " Optimierungsablauf auswählen",
    ),
    (
        " Configure – select a workflow ",
        " Konfiguration – Ablauf auswählen ",
    ),
    (
        " Enter=edit path, type path and Enter to export",
        " Eingabe=Pfad bearbeiten, Pfad eingeben und mit Eingabe exportieren",
    ),
    (" Export successful!", " Export erfolgreich!"),
    (" Metadata Services ", " Metadatendienste "),
    (
        " No bass-anchor results yet — optional step.",
        " Noch keine Bassanker-Ergebnisse — optionaler Schritt.",
    ),
    (
        " No channels yet — record some first.",
        " Noch keine Kanäle — zuerst Aufnahmen erstellen.",
    ),
    (" No data loaded", " Keine Daten geladen"),
    (
        " No probe captured yet — press \x60r\x60 to run",
        " Noch keine Sonde aufgenommen — mit \x60r\x60 starten",
    ),
    (
        " Per-Channel Alignment Delays",
        " Ausrichtungsverzögerungen pro Kanal",
    ),
    (" Recordings saved successfully!", " Aufnahmen gespeichert!"),
    (" Tab=next step", " Tab=nächster Schritt"),
    (
        " Up/Down=select  Enter=record  A=auto-record all  Tab=evaluate",
        " Hoch/Runter=auswählen  Eingabe=aufnehmen  A=alle automatisch  Tab=auswerten",
    ),
    (
        " Up/Down=select channel  Tab=save  BackTab=capture",
        " Hoch/Runter=Kanal wählen  Tab=speichern  Umschalt+Tab=aufnehmen",
    ),
    ("Align ms", "Ausrichtung ms"),
    ("Alignment Delays", "Ausrichtungsverzögerungen"),
    ("Apply Error", "Anwendungsfehler"),
    ("Apply Status", "Anwendungsstatus"),
    ("Apply", "Anwenden"),
    ("Arrival ms", "Ankunft ms"),
    ("Bass Anchor", "Bassanker"),
    ("Channel", "Kanal"),
    (
        "Channels (Up/Down to select)",
        "Kanäle (Hoch/Runter zum Wählen)",
    ),
    ("Channels", "Kanäle"),
    ("Channels: ", "Kanäle: "),
    ("Configuration", "Konfiguration"),
    ("Configure", "Konfigurieren"),
    ("Delay Probe Capture", "Verzögerungssonde aufnehmen"),
    ("Delay ms", "Verzögerung ms"),
    ("Delay", "Verzögerung"),
    ("Details", "Details"),
    ("Duration (s)", "Dauer (s)"),
    ("Error", "Fehler"),
    ("Evaluate", "Auswerten"),
    ("Filters", "Filter"),
    ("Freq (Hz)", "Frequenz (Hz)"),
    ("Gain (dB)", "Verstärkung (dB)"),
    ("Gain dB", "Verstärkung dB"),
    ("Headphone EQ", "Kopfhörer-EQ"),
    ("Loss History", "Verlustverlauf"),
    ("Loss", "Verlust"),
    (
        "Manual album/track metadata edits use the shared metadata controller.",
        "Manuelle Metadatenänderungen verwenden die gemeinsame Metadatensteuerung.",
    ),
    ("Measurement: ", "Messung: "),
    ("Mic input channel", "Mikrofon-Eingangskanal"),
    (
        "MusicBrainz search/import is anonymous by default; login is optional.",
        "MusicBrainz-Suche und -Import sind standardmäßig anonym; Anmeldung ist optional.",
    ),
    ("No log messages yet", "Noch keine Protokollmeldungen"),
    (
        "No optimization results yet. Go to Optimize step first.",
        "Noch keine Optimierungsergebnisse. Zuerst den Schritt Optimieren öffnen.",
    ),
    (
        "No recordings completed yet. Go to Capture step.",
        "Noch keine Aufnahmen abgeschlossen. Zum Aufnahmeschritt wechseln.",
    ),
    (
        "No results yet. Go to Optimize step and run optimization.",
        "Noch keine Ergebnisse. Optimierung im Schritt Optimieren starten.",
    ),
    ("OK", "OK"),
    ("Optimization", "Optimierung"),
    ("Output channel", "Ausgabekanal"),
    ("PEQ Filters", "PEQ-Filter"),
    ("Phase °", "Phase °"),
    ("Points", "Punkte"),
    ("Post Score", "Wert danach"),
    ("Pre Score", "Wert davor"),
    ("Probe duration (ms)", "Sondendauer (ms)"),
    ("Process", "Verarbeiten"),
    ("Progress", "Fortschritt"),
    ("Q", "Q"),
    ("Quality", "Qualität"),
    ("Recorded Channels", "Aufgenommene Kanäle"),
    ("Recording", "Aufnahme"),
    ("Reference freq (Hz)", "Referenzfrequenz (Hz)"),
    ("Reported dBSPL", "Gemeldeter dB SPL"),
    ("Result", "Ergebnis"),
    ("Results", "Ergebnisse"),
    ("Review", "Prüfen"),
    ("Room EQ", "Raum-EQ"),
    ("SNR dB", "SNR dB"),
    ("SPL Calibration", "SPL-Kalibrierung"),
    ("Silence gap (ms)", "Stilleabstand (ms)"),
    (
        "Source (Left/Right to toggle)",
        "Quelle (Links/Rechts zum Umschalten)",
    ),
    ("Speaker: ", "Lautsprecher: "),
    ("Spinorama EQ", "Spinorama-EQ"),
    ("Stab °", "Stabilität °"),
    ("State", "Zustand"),
    ("Status", "Status"),
    ("Suggestions (spinorama.org)", "Vorschläge (spinorama.org)"),
    ("Summary", "Zusammenfassung"),
    (
        "Target Preset (Left/Right to cycle)",
        "Ziel-Preset (Links/Rechts zum Wechseln)",
    ),
    ("Tone amplitude (0-1)", "Tonamplitude (0-1)"),
    ("Type", "Typ"),
    ("Update Plugin", "Plugin aktualisieren"),
    ("[ Run Probe ]", "[ Sonde starten ]"),
    ("|mag|", "|Betrag|"),
];

const SPANISH_UI: &[(&str, &str)] = &[
    ("Loudness", "Sonoridad"),
    ("True Peak", "Pico verdadero"),
    ("Stereo width", "Anchura estéreo"),
    ("No audio playing", "No se está reproduciendo audio"),
    ("No audio", "Sin audio"),
    ("Levels", "Niveles"),
    ("No channels", "Sin canales"),
    ("Levels (help: ?)", "Niveles (ayuda: ?)"),
    (
        " 1=Simple  2=Full  3/4/5=layout  Tab=next step",
        " 1=Simple  2=Completo  3/4/5=diseño  Tab=paso siguiente",
    ),
    (" = cancel", " = cancelar"),
    (
        " Choose your optimization workflow",
        " Elija el flujo de optimización",
    ),
    (
        " Configure – select a workflow ",
        " Configuración – elija un flujo ",
    ),
    (
        " Enter=edit path, type path and Enter to export",
        " Intro=editar ruta, escríbala e Intro para exportar",
    ),
    (" Export successful!", " ¡Exportación correcta!"),
    (" Metadata Services ", " Servicios de metadatos "),
    (
        " No bass-anchor results yet — optional step.",
        " Aún no hay resultados del anclaje de graves — paso opcional.",
    ),
    (
        " No channels yet — record some first.",
        " Aún no hay canales — realice primero algunas grabaciones.",
    ),
    (" No data loaded", " No hay datos cargados"),
    (
        " No probe captured yet — press \x60r\x60 to run",
        " Aún no se ha capturado la sonda — pulse \x60r\x60 para ejecutarla",
    ),
    (
        " Per-Channel Alignment Delays",
        " Retardos de alineación por canal",
    ),
    (
        " Recordings saved successfully!",
        " ¡Grabaciones guardadas!",
    ),
    (" Tab=next step", " Tab=paso siguiente"),
    (
        " Up/Down=select  Enter=record  A=auto-record all  Tab=evaluate",
        " Arriba/Abajo=seleccionar  Intro=grabar  A=grabar todo  Tab=evaluar",
    ),
    (
        " Up/Down=select channel  Tab=save  BackTab=capture",
        " Arriba/Abajo=elegir canal  Tab=guardar  Mayús+Tab=capturar",
    ),
    ("Align ms", "Alineación ms"),
    ("Alignment Delays", "Retardos de alineación"),
    ("Apply Error", "Error de aplicación"),
    ("Apply Status", "Estado de aplicación"),
    ("Apply", "Aplicar"),
    ("Arrival ms", "Llegada ms"),
    ("Bass Anchor", "Anclaje de graves"),
    ("Channel", "Canal"),
    (
        "Channels (Up/Down to select)",
        "Canales (Arriba/Abajo para elegir)",
    ),
    ("Channels", "Canales"),
    ("Channels: ", "Canales: "),
    ("Configuration", "Configuración"),
    ("Configure", "Configurar"),
    ("Delay Probe Capture", "Captura de sonda de retardo"),
    ("Delay ms", "Retardo ms"),
    ("Delay", "Retardo"),
    ("Details", "Detalles"),
    ("Duration (s)", "Duración (s)"),
    ("Error", "Error"),
    ("Evaluate", "Evaluar"),
    ("Filters", "Filtros"),
    ("Freq (Hz)", "Frec. (Hz)"),
    ("Gain (dB)", "Ganancia (dB)"),
    ("Gain dB", "Ganancia dB"),
    ("Headphone EQ", "EQ de auriculares"),
    ("Loss History", "Historial de pérdida"),
    ("Loss", "Pérdida"),
    (
        "Manual album/track metadata edits use the shared metadata controller.",
        "Las ediciones manuales de metadatos usan el controlador compartido.",
    ),
    ("Measurement: ", "Medición: "),
    ("Mic input channel", "Canal de entrada del micrófono"),
    (
        "MusicBrainz search/import is anonymous by default; login is optional.",
        "La búsqueda e importación de MusicBrainz es anónima por defecto; iniciar sesión es opcional.",
    ),
    ("No log messages yet", "Aún no hay mensajes de registro"),
    (
        "No optimization results yet. Go to Optimize step first.",
        "Aún no hay resultados. Vaya primero al paso Optimizar.",
    ),
    (
        "No recordings completed yet. Go to Capture step.",
        "Aún no hay grabaciones terminadas. Vaya al paso Captura.",
    ),
    (
        "No results yet. Go to Optimize step and run optimization.",
        "Aún no hay resultados. Ejecute la optimización en el paso Optimizar.",
    ),
    ("OK", "OK"),
    ("Optimization", "Optimización"),
    ("Output channel", "Canal de salida"),
    ("PEQ Filters", "Filtros PEQ"),
    ("Phase °", "Fase °"),
    ("Points", "Puntos"),
    ("Post Score", "Puntuación posterior"),
    ("Pre Score", "Puntuación previa"),
    ("Probe duration (ms)", "Duración de la sonda (ms)"),
    ("Process", "Procesar"),
    ("Progress", "Progreso"),
    ("Q", "Q"),
    ("Quality", "Calidad"),
    ("Recorded Channels", "Canales grabados"),
    ("Recording", "Grabación"),
    ("Reference freq (Hz)", "Frec. de referencia (Hz)"),
    ("Reported dBSPL", "dB SPL indicado"),
    ("Result", "Resultado"),
    ("Results", "Resultados"),
    ("Review", "Revisar"),
    ("Room EQ", "EQ de sala"),
    ("SNR dB", "SNR dB"),
    ("SPL Calibration", "Calibración SPL"),
    ("Silence gap (ms)", "Intervalo de silencio (ms)"),
    (
        "Source (Left/Right to toggle)",
        "Fuente (Izquierda/Derecha para cambiar)",
    ),
    ("Speaker: ", "Altavoz: "),
    ("Spinorama EQ", "EQ Spinorama"),
    ("Stab °", "Estab. °"),
    ("State", "Estado"),
    ("Status", "Estado"),
    ("Suggestions (spinorama.org)", "Sugerencias (spinorama.org)"),
    ("Summary", "Resumen"),
    (
        "Target Preset (Left/Right to cycle)",
        "Objetivo (Izquierda/Derecha para cambiar)",
    ),
    ("Tone amplitude (0-1)", "Amplitud del tono (0-1)"),
    ("Type", "Tipo"),
    ("Update Plugin", "Actualizar complemento"),
    ("[ Run Probe ]", "[ Ejecutar sonda ]"),
    ("|mag|", "|mag|"),
];

const DYNAMIC_TRANSLATIONS: &[DynamicTranslation] = &[
    dynamic_translation(
        "Frequency band",
        "Bande de fréquences",
        "Frequenzband",
        "Banda de frecuencia",
    ),
    dynamic_translation(
        "Boost or cut",
        "Accentuation ou atténuation",
        "Anhebung oder Absenkung",
        "Realce o recorte",
    ),
    dynamic_translation(
        "Gain amount",
        "Valeur du gain",
        "Pegelbetrag",
        "Cantidad de ganancia",
    ),
    dynamic_translation("Boost", "Accentuation", "Anhebung", "Realce"),
    dynamic_translation("Cut", "Atténuation", "Absenkung", "Recorte"),
    dynamic_translation(
        "Boost + cut",
        "Accentuation + atténuation",
        "Anhebung + Absenkung",
        "Realce + recorte",
    ),
    dynamic_translation("Foundations", "Fondamentaux", "Grundlagen", "Fundamentos"),
    dynamic_translation(
        "Frequency regions",
        "Régions fréquentielles",
        "Frequenzbereiche",
        "Regiones de frecuencia",
    ),
    dynamic_translation(
        "Hearing cuts",
        "Entendre les atténuations",
        "Absenkungen hören",
        "Escuchar recortes",
    ),
    dynamic_translation("Fine bands", "Bandes fines", "Feine Bänder", "Bandas finas"),
    dynamic_translation("Mastery", "Maîtrise", "Meisterschaft", "Dominio"),
    dynamic_translation(
        "Start with Foundations at 12 dB.",
        "Commencez par les Fondamentaux à 12 dB.",
        "Mit Grundlagen bei 12 dB beginnen.",
        "Empiece con Fundamentos a 12 dB.",
    ),
    dynamic_translation(
        "Focus around {} Hz, then reduce gain by 3 dB.",
        "Travaillez autour de {} Hz, puis réduisez le gain de 3 dB.",
        "Auf den Bereich um {} Hz konzentrieren, dann den Pegel um 3 dB senken.",
        "Concéntrese alrededor de {} Hz y reduzca la ganancia 3 dB.",
    ),
    dynamic_translation(
        "Try a boost/cut identification session.",
        "Essayez une session d’identification accentuation/atténuation.",
        "Eine Sitzung zur Erkennung von Anhebung/Absenkung versuchen.",
        "Pruebe una sesión de identificación de realce/recorte.",
    ),
    dynamic_translation(
        "No track selected",
        "Aucune piste sélectionnée",
        "Kein Titel ausgewählt",
        "Ninguna pista seleccionada",
    ),
    dynamic_translation("not set", "non définie", "nicht gesetzt", "sin definir"),
    dynamic_translation("Exercise: ", "Exercice : ", "Übung: ", "Ejercicio: "),
    dynamic_translation("Bands:", "Bandes :", "Bänder:", "Bandas:"),
    dynamic_translation("Gain:", "Gain :", "Pegel:", "Ganancia:"),
    dynamic_translation("Trials:", "Essais :", "Runden:", "Pruebas:"),
    dynamic_translation("Change:", "Modification :", "Änderung:", "Cambio:"),
    dynamic_translation("Adaptive:", "Adaptatif :", "Adaptiv:", "Adaptativo:"),
    dynamic_translation("Source:", "Source :", "Quelle:", "Fuente:"),
    dynamic_translation("Loop:", "Boucle :", "Schleife:", "Bucle:"),
    dynamic_translation(
        "e exercise · a adaptive · c change",
        "e exercice · a adaptatif · c modification",
        "e Übung · a adaptiv · c Änderung",
        "e ejercicio · a adaptativo · c cambio",
    ),
    dynamic_translation(
        "b/B bands · g/G gain · v/V Q · t/T trials",
        "b/B bandes · g/G gain · v/V Q · t/T essais",
        "b/B Bänder · g/G Pegel · v/V Q · t/T Runden",
        "b/B bandas · g/G ganancia · v/V Q · t/T pruebas",
    ),
    dynamic_translation(
        "i add source · ,/. source · [/] loop · \\ toggle",
        "i ajouter source · ,/. source · [/] boucle · \\ activer",
        "i Quelle hinzu · ,/. Quelle · [/] Schleife · \\ umschalten",
        "i añadir fuente · ,/. fuente · [/] bucle · \\ activar",
    ),
    dynamic_translation(
        "Practice setup",
        "Configuration de la pratique",
        "Übungseinstellungen",
        "Configuración de práctica",
    ),
    dynamic_translation(
        "Press s to start",
        "Appuyez sur s pour commencer",
        "s zum Starten drücken",
        "Pulse s para empezar",
    ),
    dynamic_translation(
        "{}   1 original · 2 filtered · ←/→ answer · Enter submit/next",
        "{}   1 original · 2 filtré · ←/→ réponse · Entrée valider/suivant",
        "{}   1 Original · 2 gefiltert · ←/→ Antwort · Enter senden/weiter",
        "{}   1 original · 2 filtrado · ←/→ respuesta · Intro enviar/siguiente",
    ),
    dynamic_translation(
        "EQ change",
        "Modification EQ",
        "EQ-Änderung",
        "Cambio de EQ",
    ),
    dynamic_translation(
        "Listen before answering",
        "Écoutez avant de répondre",
        "Vor der Antwort anhören",
        "Escuche antes de responder",
    ),
    dynamic_translation(
        "Start a session to reveal answer choices",
        "Démarrez une session pour afficher les réponses",
        "Sitzung starten, um Antworten zu sehen",
        "Inicie una sesión para ver las respuestas",
    ),
    dynamic_translation(
        "Your answer",
        "Votre réponse",
        "Ihre Antwort",
        "Su respuesta",
    ),
    dynamic_translation(
        "Guided courses · ↑/↓ select · Enter start",
        "Cours guidés · ↑/↓ choisir · Entrée démarrer",
        "Geführte Kurse · ↑/↓ wählen · Enter starten",
        "Cursos guiados · ↑/↓ elegir · Intro iniciar",
    ),
    dynamic_translation("Sessions:", "Sessions :", "Sitzungen:", "Sesiones:"),
    dynamic_translation("Accuracy:", "Précision :", "Genauigkeit:", "Precisión:"),
    dynamic_translation(
        "70% streak:",
        "Série à 70 % :",
        "70%-Serie:",
        "Racha del 70 %:",
    ),
    dynamic_translation("Coach:", "Conseil :", "Coach:", "Consejo:"),
    dynamic_translation(
        "Progress and coaching",
        "Progression et conseils",
        "Fortschritt und Coaching",
        "Progreso y consejos",
    ),
    dynamic_translation(
        "Recent sessions",
        "Sessions récentes",
        "Letzte Sitzungen",
        "Sesiones recientes",
    ),
    dynamic_translation(
        "APO files can only be loaded for EQ plugins",
        "Les fichiers APO ne peuvent être chargés que pour les modules d’EQ",
        "APO-Dateien können nur für EQ-Plugins geladen werden",
        "Los archivos APO solo se pueden cargar en complementos de EQ",
    ),
    dynamic_translation(
        "SOFA files can only be loaded for Binaural Decoder plugins",
        "Les fichiers SOFA ne peuvent être chargés que pour les modules de décodage binaural",
        "SOFA-Dateien können nur für Binaural-Decoder-Plugins geladen werden",
        "Los archivos SOFA solo se pueden cargar en complementos de decodificación binaural",
    ),
    dynamic_translation(
        "IR files can only be loaded for Convolution plugins",
        "Les fichiers de réponse impulsionnelle ne peuvent être chargés que pour les modules de convolution",
        "IR-Dateien können nur für Faltungs-Plugins geladen werden",
        "Los archivos de respuesta al impulso solo se pueden cargar en complementos de convolución",
    ),
    dynamic_translation(
        "APO file loaded successfully",
        "Fichier APO chargé avec succès",
        "APO-Datei erfolgreich geladen",
        "Archivo APO cargado correctamente",
    ),
    dynamic_translation(
        "Failed to load APO file: {}",
        "Échec du chargement du fichier APO : {}",
        "APO-Datei konnte nicht geladen werden: {}",
        "No se pudo cargar el archivo APO: {}",
    ),
    dynamic_translation(
        "SOFA file path set successfully",
        "Chemin du fichier SOFA défini avec succès",
        "SOFA-Dateipfad erfolgreich gesetzt",
        "Ruta del archivo SOFA establecida correctamente",
    ),
    dynamic_translation(
        "Failed to set SOFA file: {}",
        "Échec de définition du fichier SOFA : {}",
        "SOFA-Datei konnte nicht gesetzt werden: {}",
        "No se pudo establecer el archivo SOFA: {}",
    ),
    dynamic_translation(
        "No directories to scan",
        "Aucun dossier à analyser",
        "Keine Ordner zum Scannen",
        "No hay carpetas que escanear",
    ),
    dynamic_translation(
        "Starting library scan...",
        "Démarrage de l’analyse de la bibliothèque…",
        "Mediathek-Scan wird gestartet…",
        "Iniciando escaneo de la biblioteca…",
    ),
    dynamic_translation(
        "Starting FORCE library scan (all files)...",
        "Démarrage de l’analyse FORCÉE de la bibliothèque (tous les fichiers)…",
        "ERZWUNGENER Mediathek-Scan wird gestartet (alle Dateien)…",
        "Iniciando escaneo FORZADO de la biblioteca (todos los archivos)…",
    ),
    dynamic_translation(
        "Scanning: {} tracks, {} albums found...",
        "Analyse : {} pistes et {} albums trouvés…",
        "Scan: {} Titel und {} Alben gefunden…",
        "Escaneando: {} pistas y {} álbumes encontrados…",
    ),
    dynamic_translation(
        "Scan complete: {} tracks in {} albums",
        "Analyse terminée : {} pistes dans {} albums",
        "Scan abgeschlossen: {} Titel in {} Alben",
        "Escaneo completo: {} pistas en {} álbumes",
    ),
    dynamic_translation(
        "Scan failed: {}",
        "Échec de l’analyse : {}",
        "Scan fehlgeschlagen: {}",
        "Error de escaneo: {}",
    ),
    dynamic_translation(
        "Scanning library...",
        "Analyse de la bibliothèque…",
        "Mediathek wird gescannt…",
        "Escaneando la biblioteca…",
    ),
    dynamic_translation(
        "ReplayGain scan complete: {}/{} succeeded, {} failed",
        "Analyse ReplayGain terminée : {}/{} réussies, {} échecs",
        "ReplayGain-Scan abgeschlossen: {}/{} erfolgreich, {} fehlgeschlagen",
        "Escaneo ReplayGain completo: {}/{} correctas, {} fallidas",
    ),
    dynamic_translation(
        "Force waveform rescan started...",
        "Réanalyse forcée des formes d’onde démarrée…",
        "Erzwungener Wellenform-Scan gestartet…",
        "Reescaneo forzado de formas de onda iniciado…",
    ),
    dynamic_translation(
        "Bliss scan complete: {}/{} succeeded, {} failed",
        "Analyse Bliss terminée : {}/{} réussies, {} échecs",
        "Bliss-Scan abgeschlossen: {}/{} erfolgreich, {} fehlgeschlagen",
        "Escaneo Bliss completo: {}/{} correctas, {} fallidas",
    ),
    dynamic_translation(
        "Scan already in progress",
        "Analyse déjà en cours",
        "Scan läuft bereits",
        "Ya hay un escaneo en curso",
    ),
    dynamic_translation(
        "Computing album ReplayGain...",
        "Calcul du ReplayGain des albums…",
        "Album-ReplayGain wird berechnet…",
        "Calculando ReplayGain de álbumes…",
    ),
    dynamic_translation(
        "All tracks already have ReplayGain data",
        "Toutes les pistes ont déjà des données ReplayGain",
        "Alle Titel haben bereits ReplayGain-Daten",
        "Todas las pistas ya tienen datos ReplayGain",
    ),
    dynamic_translation(
        "Analyzing {} tracks for ReplayGain...",
        "Analyse ReplayGain de {} pistes…",
        "{} Titel werden für ReplayGain analysiert…",
        "Analizando {} pistas con ReplayGain…",
    ),
    dynamic_translation(
        "Bliss scan already in progress",
        "Analyse Bliss déjà en cours",
        "Bliss-Scan läuft bereits",
        "Ya hay un escaneo Bliss en curso",
    ),
    dynamic_translation(
        "All tracks already have bliss analysis data",
        "Toutes les pistes ont déjà des données d’analyse Bliss",
        "Alle Titel haben bereits Bliss-Analysedaten",
        "Todas las pistas ya tienen datos de análisis Bliss",
    ),
    dynamic_translation(
        "Analyzing {} tracks for bliss audio features...",
        "Analyse des caractéristiques audio Bliss de {} pistes…",
        "{} Titel werden auf Bliss-Audiomerkmale analysiert…",
        "Analizando características de audio Bliss de {} pistas…",
    ),
    dynamic_translation(
        "Directory added. Press 's' to scan.",
        "Dossier ajouté. Appuyez sur « s » pour l’analyser.",
        "Ordner hinzugefügt. Mit „s“ scannen.",
        "Carpeta añadida. Pulse «s» para escanear.",
    ),
    dynamic_translation(
        "Directory already exists.",
        "Le dossier existe déjà.",
        "Ordner ist bereits vorhanden.",
        "La carpeta ya existe.",
    ),
    dynamic_translation(
        "Starting database maintenance...",
        "Démarrage de la maintenance de la base de données…",
        "Datenbankwartung wird gestartet…",
        "Iniciando mantenimiento de la base de datos…",
    ),
    dynamic_translation(
        "Cleaned {} missing tracks from database",
        "{} pistes manquantes retirées de la base de données",
        "{} fehlende Titel aus der Datenbank entfernt",
        "Se eliminaron {} pistas ausentes de la base de datos",
    ),
    dynamic_translation(
        "Database is clean - no missing tracks found",
        "La base de données est propre — aucune piste manquante",
        "Datenbank ist sauber – keine fehlenden Titel gefunden",
        "La base de datos está limpia: no hay pistas ausentes",
    ),
    dynamic_translation(
        "Database maintenance failed: {}",
        "Échec de la maintenance de la base de données : {}",
        "Datenbankwartung fehlgeschlagen: {}",
        "Error de mantenimiento de la base de datos: {}",
    ),
    dynamic_translation(
        "Preset '{}': {} plugin(s) skipped",
        "Préréglage « {} » : {} module(s) ignoré(s)",
        "Preset „{}“: {} Plugin(s) übersprungen",
        "Preajuste «{}»: se omitieron {} complemento(s)",
    ),
    dynamic_translation(
        "Error: No filename specified",
        "Erreur : aucun nom de fichier indiqué",
        "Fehler: Kein Dateiname angegeben",
        "Error: no se indicó nombre de archivo",
    ),
    dynamic_translation(
        "Warning: Overwriting existing preset: {}",
        "Avertissement : remplacement du préréglage existant : {}",
        "Warnung: Vorhandenes Preset wird überschrieben: {}",
        "Aviso: se sobrescribirá el preajuste existente: {}",
    ),
    dynamic_translation(
        "Error: Could not find presets directory",
        "Erreur : dossier des préréglages introuvable",
        "Fehler: Preset-Ordner nicht gefunden",
        "Error: no se encontró la carpeta de preajustes",
    ),
    dynamic_translation(
        "Saved preset: {}",
        "Préréglage enregistré : {}",
        "Preset gespeichert: {}",
        "Preajuste guardado: {}",
    ),
    dynamic_translation(
        "Error saving: {}",
        "Erreur d’enregistrement : {}",
        "Fehler beim Speichern: {}",
        "Error al guardar: {}",
    ),
    dynamic_translation(
        "No presets available",
        "Aucun préréglage disponible",
        "Keine Presets verfügbar",
        "No hay preajustes disponibles",
    ),
    dynamic_translation(
        "Overwritten preset: {}",
        "Préréglage remplacé : {}",
        "Preset überschrieben: {}",
        "Preajuste sobrescrito: {}",
    ),
    dynamic_translation(
        "Loaded preset: {}",
        "Préréglage chargé : {}",
        "Preset geladen: {}",
        "Preajuste cargado: {}",
    ),
    dynamic_translation(
        "Loaded preset: {} ({} plugin(s) skipped)",
        "Préréglage chargé : {} ({} module(s) ignoré(s))",
        "Preset geladen: {} ({} Plugin(s) übersprungen)",
        "Preajuste cargado: {} ({} complemento(s) omitido(s))",
    ),
    dynamic_translation(
        "Error loading: {}",
        "Erreur de chargement : {}",
        "Fehler beim Laden: {}",
        "Error al cargar: {}",
    ),
    dynamic_translation(
        "Error loading preset: {}",
        "Erreur de chargement du préréglage : {}",
        "Fehler beim Laden des Presets: {}",
        "Error al cargar el preajuste: {}",
    ),
    dynamic_translation(
        "SOFA file loaded",
        "Fichier SOFA chargé",
        "SOFA-Datei geladen",
        "Archivo SOFA cargado",
    ),
    dynamic_translation(
        "IR file set",
        "Fichier de réponse impulsionnelle défini",
        "IR-Datei gesetzt",
        "Archivo de respuesta al impulso establecido",
    ),
    dynamic_translation(
        "APO error: {}",
        "Erreur APO : {}",
        "APO-Fehler: {}",
        "Error APO: {}",
    ),
    dynamic_translation(
        "APO file loaded",
        "Fichier APO chargé",
        "APO-Datei geladen",
        "Archivo APO cargado",
    ),
    dynamic_translation(
        "Config loaded from {}",
        "Configuration chargée depuis {}",
        "Konfiguration aus {} geladen",
        "Configuración cargada desde {}",
    ),
    dynamic_translation(
        "Invalid preset: {}",
        "Préréglage invalide : {}",
        "Ungültiges Preset: {}",
        "Preajuste no válido: {}",
    ),
    dynamic_translation(
        "Failed to read config: {}",
        "Échec de lecture de la configuration : {}",
        "Konfiguration konnte nicht gelesen werden: {}",
        "No se pudo leer la configuración: {}",
    ),
    dynamic_translation(
        "Imported playlist '{}'",
        "Liste « {} » importée",
        "Wiedergabeliste „{}“ importiert",
        "Lista «{}» importada",
    ),
    dynamic_translation(
        "Import error: {}",
        "Erreur d’importation : {}",
        "Importfehler: {}",
        "Error de importación: {}",
    ),
    dynamic_translation(
        "Exported to '{}'",
        "Exporté vers « {} »",
        "Nach „{}“ exportiert",
        "Exportado a «{}»",
    ),
    dynamic_translation(
        "Export error: {}",
        "Erreur d’exportation : {}",
        "Exportfehler: {}",
        "Error de exportación: {}",
    ),
    dynamic_translation(
        "A federation scan is already running.",
        "Une analyse fédérée est déjà en cours.",
        "Ein Föderationsscan läuft bereits.",
        "Ya hay un escaneo federado en curso.",
    ),
    dynamic_translation(
        "Scanning {}...",
        "Analyse de {}…",
        "{} wird gescannt…",
        "Escaneando {}…",
    ),
    dynamic_translation(
        "Federation scan failed: {}",
        "Échec de l’analyse fédérée : {}",
        "Föderationsscan fehlgeschlagen: {}",
        "Error de escaneo federado: {}",
    ),
    dynamic_translation(
        "Scan complete: {} albums, {} tracks merged.",
        "Analyse terminée : {} albums et {} pistes fusionnés.",
        "Scan abgeschlossen: {} Alben und {} Titel zusammengeführt.",
        "Escaneo completo: {} álbumes y {} pistas combinados.",
    ),
    dynamic_translation(
        "Metadata: {}",
        "Métadonnées : {}",
        "Metadaten: {}",
        "Metadatos: {}",
    ),
    dynamic_translation(
        "Added '{}' to playlist",
        "« {} » ajouté à la liste",
        "„{}“ zur Wiedergabeliste hinzugefügt",
        "«{}» añadido a la lista",
    ),
    dynamic_translation(
        "Open a playlist first (Y screen)",
        "Ouvrez d’abord une liste de lecture (écran Y)",
        "Zuerst eine Wiedergabeliste öffnen (Bildschirm Y)",
        "Abra primero una lista (pantalla Y)",
    ),
    dynamic_translation(
        "No playlist to export",
        "Aucune liste de lecture à exporter",
        "Keine Wiedergabeliste zum Exportieren",
        "No hay lista que exportar",
    ),
    dynamic_translation(
        "Created '{}'",
        "« {} » créée",
        "„{}“ erstellt",
        "«{}» creada",
    ),
    dynamic_translation(
        "Renamed to '{}'",
        "Renommée « {} »",
        "In „{}“ umbenannt",
        "Renombrada a «{}»",
    ),
    dynamic_translation(
        "Playlist deleted",
        "Liste de lecture supprimée",
        "Wiedergabeliste gelöscht",
        "Lista eliminada",
    ),
    dynamic_translation(
        "Playlist is empty",
        "La liste de lecture est vide",
        "Wiedergabeliste ist leer",
        "La lista está vacía",
    ),
    dynamic_translation(
        "Playlist already in queue ({} track{})",
        "Liste déjà dans la file ({} piste{})",
        "Wiedergabeliste bereits in der Warteschlange ({} Titel{})",
        "La lista ya está en la cola ({} pista{})",
    ),
    dynamic_translation(
        "Error starting ReplayGain scan: {}",
        "Erreur au démarrage de l’analyse ReplayGain : {}",
        "Fehler beim Start des ReplayGain-Scans: {}",
        "Error al iniciar el escaneo ReplayGain: {}",
    ),
    dynamic_translation(
        "Error starting ReplayGain force scan: {}",
        "Erreur au démarrage de l’analyse ReplayGain forcée : {}",
        "Fehler beim Start des erzwungenen ReplayGain-Scans: {}",
        "Error al iniciar el escaneo ReplayGain forzado: {}",
    ),
    dynamic_translation(
        "Error starting Bliss scan: {}",
        "Erreur au démarrage de l’analyse Bliss : {}",
        "Fehler beim Start des Bliss-Scans: {}",
        "Error al iniciar el escaneo Bliss: {}",
    ),
    dynamic_translation(
        "Error starting Bliss force scan: {}",
        "Erreur au démarrage de l’analyse Bliss forcée : {}",
        "Fehler beim Start des erzwungenen Bliss-Scans: {}",
        "Error al iniciar el escaneo Bliss forzado: {}",
    ),
    dynamic_translation(
        "Error starting Waveform scan: {}",
        "Erreur au démarrage de l’analyse des formes d’onde : {}",
        "Fehler beim Start des Wellenform-Scans: {}",
        "Error al iniciar el escaneo de formas de onda: {}",
    ),
    dynamic_translation(
        "Error starting Waveform force scan: {}",
        "Erreur au démarrage de l’analyse forcée des formes d’onde : {}",
        "Fehler beim Start des erzwungenen Wellenform-Scans: {}",
        "Error al iniciar el escaneo forzado de formas de onda: {}",
    ),
    dynamic_translation(
        "Rescanning audio + cast devices…",
        "Réanalyse des périphériques audio et Cast…",
        "Audio- und Cast-Geräte werden neu gesucht…",
        "Reescaneando dispositivos de audio y Cast…",
    ),
    dynamic_translation(
        "Metadata updated for {} file(s)",
        "Métadonnées mises à jour pour {} fichier(s)",
        "Metadaten für {} Datei(en) aktualisiert",
        "Metadatos actualizados para {} archivo(s)",
    ),
    dynamic_translation(
        "ReplayGain: {}",
        "ReplayGain : {}",
        "ReplayGain: {}",
        "ReplayGain: {}",
    ),
    dynamic_translation(
        "ReplayGain mode: {}",
        "Mode ReplayGain : {}",
        "ReplayGain-Modus: {}",
        "Modo ReplayGain: {}",
    ),
    dynamic_translation(
        "ON (Track mode)",
        "ACTIVÉ (mode piste)",
        "EIN (Titelmodus)",
        "ACTIVADO (modo pista)",
    ),
    dynamic_translation(
        "ON (Album mode)",
        "ACTIVÉ (mode album)",
        "EIN (Albummodus)",
        "ACTIVADO (modo álbum)",
    ),
    dynamic_translation("OFF", "DÉSACTIVÉ", "AUS", "DESACTIVADO"),
    dynamic_translation(
        "Plugin update failed after {} retries. Check logs for details.",
        "Échec de mise à jour des modules après {} tentatives. Consultez les journaux.",
        "Plugin-Aktualisierung nach {} Versuchen fehlgeschlagen. Details im Protokoll.",
        "La actualización falló tras {} intentos. Consulte los registros.",
    ),
    dynamic_translation(
        "Plugin chain updated",
        "Chaîne de modules mise à jour",
        "Plugin-Kette aktualisiert",
        "Cadena de complementos actualizada",
    ),
    dynamic_translation(
        "Plugin update failed, retrying... ({}/{})",
        "Échec de mise à jour des modules, nouvelle tentative… ({}/{})",
        "Plugin-Aktualisierung fehlgeschlagen, neuer Versuch… ({}/{})",
        "La actualización falló; reintentando… ({}/{})",
    ),
    dynamic_translation(
        "Plugin update failed: {}",
        "Échec de mise à jour des modules : {}",
        "Plugin-Aktualisierung fehlgeschlagen: {}",
        "Error de actualización de complementos: {}",
    ),
    dynamic_translation(
        "Output device set to '{}'; will be used for next playback",
        "Périphérique de sortie défini sur « {} » ; utilisé à la prochaine lecture",
        "Ausgabegerät auf „{}“ gesetzt; wird bei der nächsten Wiedergabe verwendet",
        "Dispositivo de salida «{}»; se usará en la próxima reproducción",
    ),
    dynamic_translation(
        " [♥ Favorites]",
        " [♥ Favoris]",
        " [♥ Favoriten]",
        " [♥ Favoritos]",
    ),
    dynamic_translation(
        "Albums ({}){}  'a' add, 't' tree, 'f' fav, 'F' filter",
        "Albums ({}){}  « a » ajouter, « t » arbre, « f » favori, « F » filtre",
        "Alben ({}){}  „a“ hinzufügen, „t“ Baum, „f“ Favorit, „F“ Filter",
        "Álbumes ({}){}  «a» añadir, «t» árbol, «f» favorito, «F» filtro",
    ),
    dynamic_translation(
        "  └─ <unknown>",
        "  └─ <inconnu>",
        "  └─ <unbekannt>",
        "  └─ <desconocido>",
    ),
    dynamic_translation(
        "Artists ({}) - 'h/l' to expand/collapse, 'a' to add, 't' to toggle view",
        "Artistes ({}) — « h/l » développer/réduire, « a » ajouter, « t » changer de vue",
        "Künstler ({}) – „h/l“ ein-/ausklappen, „a“ hinzufügen, „t“ Ansicht wechseln",
        "Artistas ({}) — «h/l» expandir/contraer, «a» añadir, «t» cambiar vista",
    ),
    dynamic_translation("1 match", "1 correspondance", "1 Treffer", "1 coincidencia"),
    dynamic_translation(
        "{}/{} matches",
        "{}/{} correspondances",
        "{}/{} Treffer",
        "{}/{} coincidencias",
    ),
    dynamic_translation("Dir: {}", "Dossier : {}", "Ordner: {}", "Carpeta: {}"),
    dynamic_translation(
        "Enter:Select | j/k:Navigate | l/Enter:Open dir | h:Parent | H:Hidden | Esc:Cancel",
        "Entrée:Choisir | j/k:Naviguer | l/Entrée:Ouvrir dossier | h:Parent | H:Cachés | Échap:Annuler",
        "Eingabe:Auswählen | j/k:Navigieren | l/Eingabe:Ordner öffnen | h:Übergeordnet | H:Versteckt | Esc:Abbrechen",
        "Intro:Seleccionar | j/k:Navegar | l/Intro:Abrir carpeta | h:Superior | H:Ocultos | Esc:Cancelar",
    ),
    dynamic_translation(
        "Select Room EQ Measurements (JSON)",
        "Choisir les mesures d’EQ de salle (JSON)",
        "Raum-EQ-Messungen auswählen (JSON)",
        "Seleccionar mediciones de EQ de sala (JSON)",
    ),
    dynamic_translation(
        "Select Export Path (JSON)",
        "Choisir le chemin d’export (JSON)",
        "Exportpfad auswählen (JSON)",
        "Seleccionar ruta de exportación (JSON)",
    ),
    dynamic_translation(
        "Select Output Directory",
        "Choisir le dossier de sortie",
        "Ausgabeordner auswählen",
        "Seleccionar carpeta de salida",
    ),
    dynamic_translation(
        "Select Mic Calibration File",
        "Choisir le fichier de calibration du micro",
        "Mikrofon-Kalibrierdatei auswählen",
        "Seleccionar archivo de calibración del micrófono",
    ),
    dynamic_translation(
        "Select Measurement CSV",
        "Choisir le CSV de mesure",
        "Mess-CSV auswählen",
        "Seleccionar CSV de medición",
    ),
    dynamic_translation(
        "Select Custom Target CSV",
        "Choisir le CSV cible personnalisé",
        "Benutzerdefinierte Ziel-CSV auswählen",
        "Seleccionar CSV de objetivo personalizado",
    ),
    dynamic_translation(
        "Select Path A Config (JSON)",
        "Choisir la configuration du chemin A (JSON)",
        "Konfiguration für Pfad A auswählen (JSON)",
        "Seleccionar configuración de ruta A (JSON)",
    ),
    dynamic_translation(
        "Select Path B Config (JSON)",
        "Choisir la configuration du chemin B (JSON)",
        "Konfiguration für Pfad B auswählen (JSON)",
        "Seleccionar configuración de ruta B (JSON)",
    ),
    dynamic_translation(
        "Select Impulse Response (WAV)",
        "Choisir la réponse impulsionnelle (WAV)",
        "Impulsantwort auswählen (WAV)",
        "Seleccionar respuesta al impulso (WAV)",
    ),
    dynamic_translation(
        "Select SOFA File",
        "Choisir le fichier SOFA",
        "SOFA-Datei auswählen",
        "Seleccionar archivo SOFA",
    ),
    dynamic_translation(
        "Import M3U Playlist",
        "Importer une liste M3U",
        "M3U-Wiedergabeliste importieren",
        "Importar lista M3U",
    ),
    dynamic_translation(
        "Export Playlist (select directory)",
        "Exporter la liste (choisir le dossier)",
        "Wiedergabeliste exportieren (Ordner wählen)",
        "Exportar lista (seleccionar carpeta)",
    ),
    dynamic_translation(
        "Select Music Directory",
        "Choisir le dossier de musique",
        "Musikordner auswählen",
        "Seleccionar carpeta de música",
    ),
    dynamic_translation(
        "↑/↓=navigate  Enter=add  Esc=cancel",
        "↑/↓=naviguer  Entrée=ajouter  Échap=annuler",
        "↑/↓=navigieren  Eingabe=hinzufügen  Esc=abbrechen",
        "↑/↓=navegar  Intro=añadir  Esc=cancelar",
    ),
    dynamic_translation(
        "'a'=add plugins  's'=save  'l'=load",
        "« a »=ajouter des modules  « s »=enregistrer  « l »=charger",
        "„a“=Plugins hinzufügen  „s“=speichern  „l“=laden",
        "«a»=añadir complementos  «s»=guardar  «l»=cargar",
    ),
    dynamic_translation(
        "Graph mode: {} plugins, {} connections.\nNon-linear topology (parallel branches).\nUse the desktop app (sotf-desktop) to edit nodes and connections visually.",
        "Mode graphe : {} modules, {} connexions.\nTopologie non linéaire (branches parallèles).\nUtilisez l’application de bureau (sotf-desktop) pour modifier visuellement les nœuds et connexions.",
        "Graphmodus: {} Plugins, {} Verbindungen.\nNichtlineare Topologie (parallele Zweige).\nKnoten und Verbindungen können visuell in der Desktop-App (sotf-desktop) bearbeitet werden.",
        "Modo de grafo: {} complementos, {} conexiones.\nTopología no lineal (ramas paralelas).\nUse la aplicación de escritorio (sotf-desktop) para editar visualmente nodos y conexiones.",
    ),
    dynamic_translation(
        "Plugin chain (graph)",
        "Chaîne de modules (graphe)",
        "Plugin-Kette (Graph)",
        "Cadena de complementos (grafo)",
    ),
    dynamic_translation(
        "Loudness Monitor Input",
        "Entrée du moniteur de sonie",
        "Lautheitsmonitor-Eingang",
        "Entrada del monitor de sonoridad",
    ),
    dynamic_translation(
        "Loudness Monitor Output",
        "Sortie du moniteur de sonie",
        "Lautheitsmonitor-Ausgang",
        "Salida del monitor de sonoridad",
    ),
    dynamic_translation(
        "Replay Gain",
        "Gain de lecture",
        "Wiedergabeverstärkung",
        "Ganancia de reproducción",
    ),
    dynamic_translation("0 plugins", "0 module", "0 Plugins", "0 complementos"),
    dynamic_translation(
        "{} plugins ({} ch)",
        "{} modules ({} canaux)",
        "{} Plugins ({} Kanäle)",
        "{} complementos ({} canales)",
    ),
    dynamic_translation(
        "▶ Select Plugin (↑/↓, Enter=add, Esc=cancel)",
        "▶ Choisir un module (↑/↓, Entrée=ajouter, Échap=annuler)",
        "▶ Plugin auswählen (↑/↓, Eingabe=hinzufügen, Esc=abbrechen)",
        "▶ Seleccionar complemento (↑/↓, Intro=añadir, Esc=cancelar)",
    ),
    dynamic_translation(
        "Available Plugins",
        "Modules disponibles",
        "Verfügbare Plugins",
        "Complementos disponibles",
    ),
    dynamic_translation(
        "Edit {} Plugin (ESC to close)",
        "Modifier le module {} (Échap pour fermer)",
        "{}-Plugin bearbeiten (Esc zum Schließen)",
        "Editar complemento {} (Esc para cerrar)",
    ),
    dynamic_translation(
        " to select parameter, ",
        " pour choisir le paramètre, ",
        " Parameter auswählen, ",
        " para seleccionar parámetro, ",
    ),
    dynamic_translation(
        " to adjust value",
        " pour régler la valeur",
        " Wert anpassen",
        " para ajustar el valor",
    ),
    dynamic_translation(
        " Matrix Mixer - {} (ESC to close) ",
        " Mélangeur matriciel — {} (Échap pour fermer) ",
        " Matrix-Mixer – {} (Esc zum Schließen) ",
        " Mezclador matricial — {} (Esc para cerrar) ",
    ),
    dynamic_translation(
        "↑↓: Select | ←→: Adjust | Tab: Grid Mode | Esc: Exit",
        "↑↓ : Choisir | ←→ : Régler | Tab : Mode grille | Échap : Quitter",
        "↑↓: Auswählen | ←→: Anpassen | Tab: Rastermodus | Esc: Beenden",
        "↑↓: Seleccionar | ←→: Ajustar | Tab: Modo rejilla | Esc: Salir",
    ),
    dynamic_translation(
        "↑↓←→: Navigate | -/+: Adjust ±0.5dB | 0: Zero | 1: Unity | Tab: Header Mode | Esc: Exit",
        "↑↓←→ : Naviguer | -/+ : Régler ±0,5 dB | 0 : Zéro | 1 : Unité | Tab : Mode en-tête | Échap : Quitter",
        "↑↓←→: Navigieren | -/+: ±0,5 dB anpassen | 0: Null | 1: Eins | Tab: Kopfmodus | Esc: Beenden",
        "↑↓←→: Navegar | -/+: Ajustar ±0,5 dB | 0: Cero | 1: Unidad | Tab: Modo cabecera | Esc: Salir",
    ),
    dynamic_translation(
        "  Input Channels:  ",
        "  Canaux d’entrée :  ",
        "  Eingangskanäle:  ",
        "  Canales de entrada:  ",
    ),
    dynamic_translation(
        "  Output Channels: ",
        "  Canaux de sortie : ",
        "  Ausgangskanäle: ",
        "  Canales de salida: ",
    ),
    dynamic_translation(
        "  Preset:          ",
        "  Préréglage :      ",
        "  Preset:           ",
        "  Preajuste:        ",
    ),
    dynamic_translation(
        " Matrix Grid ",
        " Grille matricielle ",
        " Matrixraster ",
        " Rejilla matricial ",
    ),
    dynamic_translation(
        "Save Plugin Preset",
        "Enregistrer le préréglage de modules",
        "Plugin-Preset speichern",
        "Guardar preajuste de complementos",
    ),
    dynamic_translation(
        "Enter preset name (without .json extension):",
        "Saisissez le nom du préréglage (sans extension .json) :",
        "Preset-Namen eingeben (ohne .json-Erweiterung):",
        "Escriba el nombre del preajuste (sin extensión .json):",
    ),
    dynamic_translation(
        "  Saved to: ",
        "  Enregistré dans : ",
        "  Gespeichert unter: ",
        "  Guardado en: ",
    ),
    dynamic_translation("Note: ", "Remarque : ", "Hinweis: ", "Nota: "),
    dynamic_translation(
        ".json extension will be added automatically",
        "l’extension .json sera ajoutée automatiquement",
        ".json-Erweiterung wird automatisch hinzugefügt",
        "la extensión .json se añadirá automáticamente",
    ),
    dynamic_translation(
        "Press Enter to save, ESC to cancel",
        "Entrée pour enregistrer, Échap pour annuler",
        "Eingabe zum Speichern, Esc zum Abbrechen",
        "Intro para guardar, Esc para cancelar",
    ),
    dynamic_translation(
        "No existing presets found in plugin_presets directory",
        "Aucun préréglage trouvé dans le dossier plugin_presets",
        "Keine Presets im Ordner plugin_presets gefunden",
        "No se encontraron preajustes en la carpeta plugin_presets",
    ),
    dynamic_translation(
        "Type a preset name to save",
        "Saisissez un nom de préréglage à enregistrer",
        "Namen für das zu speichernde Preset eingeben",
        "Escriba un nombre de preajuste para guardar",
    ),
    dynamic_translation(
        "Press ESC to cancel",
        "Échap pour annuler",
        "Esc zum Abbrechen",
        "Esc para cancelar",
    ),
    dynamic_translation(
        "Existing Presets ",
        "Préréglages existants ",
        "Vorhandene Presets ",
        "Preajustes existentes ",
    ),
    dynamic_translation(
        "(↑/↓ to select, Enter to overwrite, or type new name)",
        "(↑/↓ pour choisir, Entrée pour remplacer, ou saisissez un nouveau nom)",
        "(↑/↓ auswählen, Eingabe überschreiben oder neuen Namen eingeben)",
        "(↑/↓ para seleccionar, Intro para sobrescribir o escriba un nombre nuevo)",
    ),
    dynamic_translation("Hint: ", "Conseil : ", "Tipp: ", "Consejo: "),
    dynamic_translation(
        "Select and press Enter to overwrite, or type to create new preset",
        "Choisissez puis appuyez sur Entrée pour remplacer, ou saisissez un nouveau préréglage",
        "Auswählen und mit Eingabe überschreiben oder zur Erstellung eines neuen Presets tippen",
        "Seleccione y pulse Intro para sobrescribir, o escriba para crear un preajuste nuevo",
    ),
    dynamic_translation(
        "Load Plugin Preset",
        "Charger un préréglage de modules",
        "Plugin-Preset laden",
        "Cargar preajuste de complementos",
    ),
    dynamic_translation(
        "Enter filename (without .json extension):",
        "Saisissez le nom du fichier (sans extension .json) :",
        "Dateinamen eingeben (ohne .json-Erweiterung):",
        "Escriba el nombre del archivo (sin extensión .json):",
    ),
    dynamic_translation(
        "Press Enter to load, ESC to cancel",
        "Entrée pour charger, Échap pour annuler",
        "Eingabe zum Laden, Esc zum Abbrechen",
        "Intro para cargar, Esc para cancelar",
    ),
    dynamic_translation(
        "No presets found in plugin_presets directory",
        "Aucun préréglage dans le dossier plugin_presets",
        "Keine Presets im Ordner plugin_presets gefunden",
        "No se encontraron preajustes en la carpeta plugin_presets",
    ),
    dynamic_translation("You can:", "Vous pouvez :", "Möglichkeiten:", "Puede:"),
    dynamic_translation(
        "  • Type a filename to load a preset",
        "  • Saisir un nom de fichier pour charger un préréglage",
        "  • Einen Dateinamen eingeben, um ein Preset zu laden",
        "  • Escribir un nombre de archivo para cargar un preajuste",
    ),
    dynamic_translation(
        "  • Press ESC to cancel",
        "  • Appuyer sur Échap pour annuler",
        "  • Esc zum Abbrechen drücken",
        "  • Pulsar Esc para cancelar",
    ),
    dynamic_translation(
        "  • Save your first preset with 's' from the Plugins screen",
        "  • Enregistrer votre premier préréglage avec « s » depuis l’écran Modules",
        "  • Das erste Preset mit „s“ im Plugin-Bildschirm speichern",
        "  • Guardar el primer preajuste con «s» desde la pantalla Complementos",
    ),
    dynamic_translation(
        "Available Presets ",
        "Préréglages disponibles ",
        "Verfügbare Presets ",
        "Preajustes disponibles ",
    ),
    dynamic_translation(
        "(↑/↓ to select, Enter to load)",
        "(↑/↓ pour choisir, Entrée pour charger)",
        "(↑/↓ auswählen, Eingabe laden)",
        "(↑/↓ para seleccionar, Intro para cargar)",
    ),
    dynamic_translation(
        "Or type a filename to load manually, ESC to cancel",
        "Ou saisissez un nom de fichier à charger manuellement, Échap pour annuler",
        "Oder Dateinamen zum manuellen Laden eingeben, Esc zum Abbrechen",
        "O escriba un nombre para cargar manualmente, Esc para cancelar",
    ),
    dynamic_translation(
        "Load APO EQ File",
        "Charger un fichier d’EQ APO",
        "APO-EQ-Datei laden",
        "Cargar archivo de EQ APO",
    ),
    dynamic_translation(
        "Enter path to APO file:",
        "Saisissez le chemin du fichier APO :",
        "Pfad zur APO-Datei eingeben:",
        "Escriba la ruta del archivo APO:",
    ),
    dynamic_translation(
        "Supported format:",
        "Format pris en charge :",
        "Unterstütztes Format:",
        "Formato compatible:",
    ),
    dynamic_translation(
        "Load SOFA HRTF File",
        "Charger un fichier HRTF SOFA",
        "SOFA-HRTF-Datei laden",
        "Cargar archivo HRTF SOFA",
    ),
    dynamic_translation(
        "Enter path to SOFA file containing HRTFs:",
        "Saisissez le chemin du fichier SOFA contenant les HRTF :",
        "Pfad zur SOFA-Datei mit HRTFs eingeben:",
        "Escriba la ruta del archivo SOFA que contiene las HRTF:",
    ),
    dynamic_translation(
        "SOFA format contains Head-Related Transfer Functions",
        "Le format SOFA contient des fonctions de transfert liées à la tête",
        "Das SOFA-Format enthält kopfbezogene Übertragungsfunktionen",
        "El formato SOFA contiene funciones de transferencia relacionadas con la cabeza",
    ),
    dynamic_translation(
        "Press Enter to set path, ESC to cancel",
        "Entrée pour définir le chemin, Échap pour annuler",
        "Eingabe zum Setzen des Pfads, Esc zum Abbrechen",
        "Intro para establecer la ruta, Esc para cancelar",
    ),
    dynamic_translation(
        "  Cycle through screens and level meters pane",
        "  Parcourir les écrans et le volet des niveaux",
        "  Durch Bildschirme und Pegelbereich wechseln",
        "  Recorrer pantallas y panel de niveles",
    ),
    dynamic_translation(
        "  Jump to Library/Queue/Plugins/Devices/Configure/Playlists/Tools",
        "  Aller à Bibliothèque/File d’attente/Modules/Périphériques/Configuration/Listes de lecture/Outils",
        "  Zu Mediathek/Warteschlange/Plugins/Geräte/Konfiguration/Wiedergabelisten/Werkzeuge wechseln",
        "  Ir a Biblioteca/Cola/Complementos/Dispositivos/Configuración/Listas de reproducción/Herramientas",
    ),
    dynamic_translation(
        "  Focus level meters pane",
        "  Activer le volet des niveaux",
        "  Pegelbereich fokussieren",
        "  Enfocar el panel de niveles",
    ),
    dynamic_translation(
        "  Increase volume",
        "  Augmenter le volume",
        "  Lautstärke erhöhen",
        "  Subir el volumen",
    ),
    dynamic_translation(
        "  Decrease volume",
        "  Baisser le volume",
        "  Lautstärke verringern",
        "  Bajar el volumen",
    ),
    dynamic_translation(
        "  Select output device",
        "  Choisir le périphérique de sortie",
        "  Ausgabegerät auswählen",
        "  Seleccionar dispositivo de salida",
    ),
    dynamic_translation(
        "  Navigate between channel groups",
        "  Parcourir les groupes de canaux",
        "  Zwischen Kanalgruppen navigieren",
        "  Navegar entre grupos de canales",
    ),
    dynamic_translation(
        "  Select mute/solo control",
        "  Choisir le contrôle muet/solo",
        "  Stumm/Solo-Steuerung auswählen",
        "  Seleccionar control de silencio/solo",
    ),
    dynamic_translation(
        "  Toggle mute/solo on selected group",
        "  Basculer muet/solo sur le groupe sélectionné",
        "  Stumm/Solo für gewählte Gruppe umschalten",
        "  Alternar silencio/solo en el grupo seleccionado",
    ),
    dynamic_translation(
        "  Clear all mutes and solos",
        "  Effacer tous les muets et solos",
        "  Alle Stumm- und Solo-Schaltungen aufheben",
        "  Quitar todos los silencios y solos",
    ),
    dynamic_translation(
        "  Return to main pane",
        "  Revenir au volet principal",
        "  Zum Hauptbereich zurückkehren",
        "  Volver al panel principal",
    ),
    dynamic_translation(
        "  Navigate level meter groups",
        "  Parcourir les groupes de niveaux",
        "  Durch Pegelgruppen navigieren",
        "  Navegar por grupos de niveles",
    ),
    dynamic_translation(
        "  Toggle solo on selected group",
        "  Basculer le solo du groupe sélectionné",
        "  Solo für gewählte Gruppe umschalten",
        "  Alternar solo en el grupo seleccionado",
    ),
    dynamic_translation(
        "  Toggle dim on selected group",
        "  Basculer l’atténuation du groupe sélectionné",
        "  Dämpfung für die gewählte Gruppe umschalten",
        "  Alternar atenuación en el grupo seleccionado",
    ),
    dynamic_translation(
        "  Toggle mute",
        "  Basculer le mode muet",
        "  Stummschaltung umschalten",
        "  Alternar silencio",
    ),
    dynamic_translation(
        "  Toggle ReplayGain",
        "  Basculer ReplayGain",
        "  ReplayGain umschalten",
        "  Alternar ReplayGain",
    ),
    dynamic_translation(
        "  Cycle ReplayGain mode",
        "  Changer le mode ReplayGain",
        "  ReplayGain-Modus wechseln",
        "  Cambiar el modo ReplayGain",
    ),
    dynamic_translation(
        "  Show this help",
        "  Afficher cette aide",
        "  Diese Hilfe anzeigen",
        "  Mostrar esta ayuda",
    ),
    dynamic_translation(
        "  Quit (ESC quits from main pane)",
        "  Quitter (Échap quitte depuis le volet principal)",
        "  Beenden (Esc beendet im Hauptbereich)",
        "  Salir (Esc sale desde el panel principal)",
    ),
    dynamic_translation(
        "{} keybindings",
        "Raccourcis — {}",
        "{}-Tastenkürzel",
        "Atajos de {}",
    ),
    dynamic_translation(
        " Edit Metadata ",
        " Modifier les métadonnées ",
        " Metadaten bearbeiten ",
        " Editar metadatos ",
    ),
    dynamic_translation("Target: ", "Cible : ", "Ziel: ", "Destino: "),
    dynamic_translation("Title", "Titre", "Titel", "Título"),
    dynamic_translation(
        "Album Artist",
        "Artiste de l’album",
        "Albumkünstler",
        "Artista del álbum",
    ),
    dynamic_translation("Disc", "Disque", "Disc", "Disco"),
    dynamic_translation("Track", "Piste", "Titel", "Pista"),
    dynamic_translation("Conductor", "Chef d’orchestre", "Dirigent", "Director"),
    dynamic_translation("Performer", "Interprète", "Interpret", "Intérprete"),
    dynamic_translation("ISRC", "ISRC", "ISRC", "ISRC"),
    dynamic_translation("Ensemble", "Ensemble", "Ensemble", "Conjunto"),
    dynamic_translation("Edition", "Édition", "Ausgabe", "Edición"),
    dynamic_translation("Preview: ", "Aperçu : ", "Vorschau: ", "Vista previa: "),
    dynamic_translation(
        "{} file(s), {} unsupported, sidecar {}",
        "{} fichier(s), {} non pris en charge, annexe {}",
        "{} Datei(en), {} nicht unterstützt, Sidecar {}",
        "{} archivo(s), {} no compatibles, anexo {}",
    ),
    dynamic_translation("Warning: ", "Avertissement : ", "Warnung: ", "Aviso: "),
    dynamic_translation(
        "unsupported",
        "non pris en charge",
        "nicht unterstützt",
        "no compatible",
    ),
    dynamic_translation(
        "Preview: press p before saving",
        "Aperçu : appuyez sur p avant d’enregistrer",
        "Vorschau: vor dem Speichern p drücken",
        "Vista previa: pulse p antes de guardar",
    ),
    dynamic_translation("Error: ", "Erreur : ", "Fehler: ", "Error: "),
    dynamic_translation(
        "MusicBrainz: ",
        "MusicBrainz : ",
        "MusicBrainz: ",
        "MusicBrainz: ",
    ),
    dynamic_translation(
        "Search error: ",
        "Erreur de recherche : ",
        "Suchfehler: ",
        "Error de búsqueda: ",
    ),
    dynamic_translation("Untitled", "Sans titre", "Ohne Titel", "Sin título"),
    dynamic_translation("unknown", "inconnu", "unbekannt", "desconocido"),
    dynamic_translation(
        " Type value | Enter=confirm | Esc=cancel",
        " Saisir la valeur | Entrée=confirmer | Échap=annuler",
        " Wert eingeben | Eingabe=bestätigen | Esc=abbrechen",
        " Escriba el valor | Intro=confirmar | Esc=cancelar",
    ),
    dynamic_translation(
        " ↑↓ field | Enter edit | p preview | s save | b MusicBrainz | i import | ←→ candidate | Esc close",
        " ↑↓ champ | Entrée modifier | p aperçu | s enregistrer | b MusicBrainz | i importer | ←→ candidat | Échap fermer",
        " ↑↓ Feld | Eingabe bearbeiten | p Vorschau | s speichern | b MusicBrainz | i importieren | ←→ Kandidat | Esc schließen",
        " ↑↓ campo | Intro editar | p vista previa | s guardar | b MusicBrainz | i importar | ←→ candidato | Esc cerrar",
    ),
    dynamic_translation(" Error ", " Erreur ", " Fehler ", " Error "),
    dynamic_translation(
        "Audio Playback Error",
        "Erreur de lecture audio",
        "Fehler bei der Audiowiedergabe",
        "Error de reproducción de audio",
    ),
    dynamic_translation("Press ", "Appuyez sur ", "Drücken Sie ", "Pulse "),
    dynamic_translation(", or ", ", ou ", ", oder ", ", o "),
    dynamic_translation(
        " to close",
        " pour fermer",
        " zum Schließen",
        " para cerrar",
    ),
    dynamic_translation(
        " Channel Conflict ",
        " Conflit de canaux ",
        " Kanalkonflikt ",
        " Conflicto de canales ",
    ),
    dynamic_translation(
        "This track has {} channels but these plugins",
        "Cette piste a {} canaux, mais ces modules",
        "Dieser Titel hat {} Kanäle, aber diese Plugins",
        "Esta pista tiene {} canales, pero estos complementos",
    ),
    dynamic_translation(
        "are incompatible:",
        "sont incompatibles :",
        "sind inkompatibel:",
        "son incompatibles:",
    ),
    dynamic_translation(
        "  {} (requires {}ch, got {}ch)",
        "  {} (exige {} canaux, {} reçus)",
        "  {} (benötigt {} Kanäle, erhalten: {})",
        "  {} (requiere {} canales, recibió {})",
    ),
    dynamic_translation(
        "Suspend incompatible and play",
        "Suspendre les incompatibles et lire",
        "Inkompatible aussetzen und abspielen",
        "Suspender incompatibles y reproducir",
    ),
    dynamic_translation(
        "Remove incompatible and play",
        "Retirer les incompatibles et lire",
        "Inkompatible entfernen und abspielen",
        "Quitar incompatibles y reproducir",
    ),
    dynamic_translation(
        "Cancel playback",
        "Annuler la lecture",
        "Wiedergabe abbrechen",
        "Cancelar reproducción",
    ),
    dynamic_translation("Use ", "Utilisez ", "Mit ", "Use "),
    dynamic_translation(
        " to select, ",
        " pour choisir, ",
        " auswählen, ",
        " para seleccionar, ",
    ),
    dynamic_translation(
        " to confirm, ",
        " pour confirmer, ",
        " bestätigen, ",
        " para confirmar, ",
    ),
    dynamic_translation(
        " to cancel",
        " pour annuler",
        " abbrechen",
        " para cancelar",
    ),
    dynamic_translation(
        " Tab=Switch section  Up/Down=Navigate  Enter/Space=Toggle/Edit  Esc=Back",
        " Tab=Changer de section  Haut/Bas=Naviguer  Entrée/Espace=Activer/Modifier  Échap=Retour",
        " Tab=Bereich wechseln  Hoch/Runter=Navigieren  Eingabe/Leertaste=Umschalten/Bearbeiten  Esc=Zurück",
        " Tab=Cambiar sección  Arriba/Abajo=Navegar  Intro/Espacio=Activar/Editar  Esc=Volver",
    ),
    dynamic_translation(
        "(auto on enable)",
        "(auto à l’activation)",
        "(automatisch beim Aktivieren)",
        "(automático al activar)",
    ),
    dynamic_translation("YES", "OUI", "JA", "SÍ"),
    dynamic_translation(
        "Bind Address",
        "Adresse d’écoute",
        "Bind-Adresse",
        "Dirección de escucha",
    ),
    dynamic_translation(
        "Auth Token",
        "Jeton d’authentification",
        "Anmeldetoken",
        "Token de autenticación",
    ),
    dynamic_translation("  URL: {}", "  URL : {}", "  URL: {}", "  URL: {}"),
    dynamic_translation(
        "  Remote apps use this API port.",
        "  Les applications distantes utilisent ce port API.",
        "  Remote-Apps verwenden diesen API-Port.",
        "  Las aplicaciones remotas usan este puerto de API.",
    ),
    dynamic_translation(
        "  MPD clients use the MPD port.",
        "  Les clients MPD utilisent le port MPD.",
        "  MPD-Clients verwenden den MPD-Port.",
        "  Los clientes MPD usan el puerto MPD.",
    ),
    dynamic_translation(" SOTF API ", " API SOTF ", " SOTF-API ", " API de SOTF "),
    dynamic_translation("TLS", "TLS", "TLS", "TLS"),
    dynamic_translation("Certificate", "Certificat", "Zertifikat", "Certificado"),
    dynamic_translation(
        "Trusted Clients",
        "Clients approuvés",
        "Vertrauenswürdige Clients",
        "Clientes de confianza",
    ),
    dynamic_translation(
        "{} (+{} more)",
        "{} (+{} autres)",
        "{} (+{} weitere)",
        "{} (+{} más)",
    ),
    dynamic_translation(
        "  ! {} trusted client fingerprint value(s) invalid.",
        "  ! {} empreinte(s) de client approuvé invalide(s).",
        "  ! {} ungültige Fingerabdruckwerte vertrauenswürdiger Clients.",
        "  ! {} valor(es) de huella de cliente de confianza no válido(s).",
    ),
    dynamic_translation(
        "    Use client certificate SHA-256 fingerprints.",
        "    Utilisez les empreintes SHA-256 des certificats clients.",
        "    SHA-256-Fingerabdrücke der Client-Zertifikate verwenden.",
        "    Use huellas SHA-256 de certificados de cliente.",
    ),
    dynamic_translation(
        "  ! Certificate auth needs at least one trusted client",
        "  ! L’authentification par certificat exige au moins un client approuvé",
        "  ! Zertifikatsanmeldung benötigt mindestens einen vertrauenswürdigen Client",
        "  ! La autenticación por certificado necesita al menos un cliente de confianza",
    ),
    dynamic_translation(
        "    fingerprint, or switch Auth Mode to Password.",
        "    avec une empreinte, ou passez le mode d’authentification à Mot de passe.",
        "    mit Fingerabdruck; alternativ den Anmeldemodus auf Passwort stellen.",
        "    con huella, o cambie el modo de autenticación a Contraseña.",
    ),
    dynamic_translation(
        "  ! Password auth needs a non-empty password.",
        "  ! L’authentification exige un mot de passe non vide.",
        "  ! Passwortanmeldung benötigt ein nicht leeres Passwort.",
        "  ! La autenticación necesita una contraseña no vacía.",
    ),
    dynamic_translation(
        "  Pairing clients can add trust automatically.",
        "  L’association des clients peut ajouter la confiance automatiquement.",
        "  Beim Koppeln kann Client-Vertrauen automatisch hinzugefügt werden.",
        "  El emparejamiento puede añadir confianza automáticamente.",
    ),
    dynamic_translation(
        "  Manual fingerprints: comma-separated SHA-256 values.",
        "  Empreintes manuelles : valeurs SHA-256 séparées par des virgules.",
        "  Manuelle Fingerabdrücke: kommagetrennte SHA-256-Werte.",
        "  Huellas manuales: valores SHA-256 separados por comas.",
    ),
    dynamic_translation(
        "  Fingerprint: {}",
        "  Empreinte : {}",
        "  Fingerabdruck: {}",
        "  Huella: {}",
    ),
    dynamic_translation(
        " MPD Server ",
        " Serveur MPD ",
        " MPD-Server ",
        " Servidor MPD ",
    ),
    dynamic_translation(
        "  Bind 0.0.0.0 listens on all interfaces.",
        "  L’adresse 0.0.0.0 écoute sur toutes les interfaces.",
        "  Bind 0.0.0.0 lauscht auf allen Schnittstellen.",
        "  La dirección 0.0.0.0 escucha en todas las interfaces.",
    ),
    dynamic_translation(
        "  (DLNA uses plain HTTP for",
        "  (DLNA utilise HTTP non chiffré pour",
        "  (DLNA verwendet unverschlüsseltes HTTP für",
        "  (DLNA usa HTTP sin cifrar para",
    ),
    dynamic_translation(
        "   device compatibility)",
        "   la compatibilité des appareils)",
        "   Gerätekompatibilität)",
        "   compatibilidad con dispositivos)",
    ),
    dynamic_translation(
        " DLNA Server ",
        " Serveur DLNA ",
        " DLNA-Server ",
        " Servidor DLNA ",
    ),
    dynamic_translation(
        " a=Add  e/Enter=Edit  d=Delete  t=Test+Scan  s=Scan  Space=Toggle  Esc=Back",
        " a=Ajouter  e/Entrée=Modifier  d=Supprimer  t=Tester+Analyser  s=Analyser  Espace=Activer  Échap=Retour",
        " a=Hinzufügen  e/Eingabe=Bearbeiten  d=Löschen  t=Testen+Scannen  s=Scannen  Leertaste=Umschalten  Esc=Zurück",
        " a=Añadir  e/Intro=Editar  d=Eliminar  t=Probar+Escanear  s=Escanear  Espacio=Activar  Esc=Volver",
    ),
    dynamic_translation(
        " a=Add  e/Enter=Edit  d=Delete  t=Test+Scan  s=Scan  l=Login  L=Logout  Space=Toggle  Esc=Back",
        " a=Ajouter  e/Entrée=Modifier  d=Supprimer  t=Tester+Analyser  s=Analyser  l=Connexion  L=Déconnexion  Espace=Activer  Échap=Retour",
        " a=Hinzufügen  e/Eingabe=Bearbeiten  d=Löschen  t=Testen+Scannen  s=Scannen  l=Anmelden  L=Abmelden  Leertaste=Umschalten  Esc=Zurück",
        " a=Añadir  e/Intro=Editar  d=Eliminar  t=Probar+Escanear  s=Escanear  l=Entrar  L=Salir  Espacio=Activar  Esc=Volver",
    ),
    dynamic_translation(
        " Service Login ",
        " Connexion au service ",
        " Dienst-Anmeldung ",
        " Inicio de sesión del servicio ",
    ),
    dynamic_translation(
        "  Starting login...",
        "  Connexion en cours…",
        "  Anmeldung läuft…",
        "  Iniciando sesión…",
    ),
    dynamic_translation(
        "  Visit: {}",
        "  Visitez : {}",
        "  Öffnen: {}",
        "  Visite: {}",
    ),
    dynamic_translation(
        "  Code: {} (expires in {}s)",
        "  Code : {} (expire dans {} s)",
        "  Code: {} (läuft ab in {} s)",
        "  Código: {} (caduca en {} s)",
    ),
    dynamic_translation(
        "  Waiting for authorization... (l = cancel)",
        "  En attente d’autorisation… (l = annuler)",
        "  Warte auf Autorisierung… (l = abbrechen)",
        "  Esperando autorización… (l = cancelar)",
    ),
    dynamic_translation(
        "  Complete the sign-in in your browser.",
        "  Terminez la connexion dans votre navigateur.",
        "  Anmeldung im Browser abschließen.",
        "  Complete el inicio de sesión en su navegador.",
    ),
    dynamic_translation(
        "  Waiting for the browser callback... (l = cancel)",
        "  En attente du retour du navigateur… (l = annuler)",
        "  Warte auf die Browser-Rückmeldung… (l = abbrechen)",
        "  Esperando la respuesta del navegador… (l = cancelar)",
    ),
    dynamic_translation(
        "Login cancelled.",
        "Connexion annulée.",
        "Anmeldung abgebrochen.",
        "Inicio de sesión cancelado.",
    ),
    dynamic_translation(
        "Login is only available for Tidal and Spotify sources.",
        "La connexion n’est disponible que pour les sources Tidal et Spotify.",
        "Anmeldung ist nur für Tidal- und Spotify-Quellen verfügbar.",
        "El inicio de sesión solo está disponible para fuentes Tidal y Spotify.",
    ),
    dynamic_translation(
        "Logout is only available for Tidal and Spotify sources.",
        "La déconnexion n’est disponible que pour les sources Tidal et Spotify.",
        "Abmeldung ist nur für Tidal- und Spotify-Quellen verfügbar.",
        "El cierre de sesión solo está disponible para fuentes Tidal y Spotify.",
    ),
    dynamic_translation(
        "Login failed: {}",
        "Échec de la connexion : {}",
        "Anmeldung fehlgeschlagen: {}",
        "Error de inicio de sesión: {}",
    ),
    dynamic_translation(
        "Login failed unexpectedly (background thread terminated).",
        "La connexion a échoué de façon inattendue (fil d’arrière-plan terminé).",
        "Anmeldung unerwartet fehlgeschlagen (Hintergrund-Thread beendet).",
        "El inicio de sesión falló inesperadamente (hilo en segundo plano terminado).",
    ),
    dynamic_translation(
        "Failed to start Tidal login: {}",
        "Échec du démarrage de la connexion Tidal : {}",
        "Tidal-Anmeldung konnte nicht gestartet werden: {}",
        "Error al iniciar la conexión de Tidal: {}",
    ),
    dynamic_translation(
        "Failed to start Spotify login: {}",
        "Échec du démarrage de la connexion Spotify : {}",
        "Spotify-Anmeldung konnte nicht gestartet werden: {}",
        "Error al iniciar la conexión de Spotify: {}",
    ),
    dynamic_translation(
        "Failed to save source: {}",
        "Échec de l’enregistrement de la source : {}",
        "Quelle konnte nicht gespeichert werden: {}",
        "Error al guardar la fuente: {}",
    ),
    dynamic_translation(
        "Logged out of Tidal — tokens cleared for '{}'.",
        "Déconnecté de Tidal — jetons effacés pour « {} ».",
        "Von Tidal abgemeldet — Token für „{}“ gelöscht.",
        "Sesión de Tidal cerrada — tokens eliminados para «{}».",
    ),
    dynamic_translation(
        "Logged out of Spotify (cached credentials removed).",
        "Déconnecté de Spotify (identifiants en cache supprimés).",
        "Von Spotify abgemeldet (zwischengespeicherte Anmeldedaten entfernt).",
        "Sesión de Spotify cerrada (credenciales en caché eliminadas).",
    ),
    dynamic_translation(
        "Spotify logout: no cached credentials to remove.",
        "Déconnexion Spotify : aucun identifiant en cache à supprimer.",
        "Spotify-Abmeldung: keine zwischengespeicherten Anmeldedaten vorhanden.",
        "Cierre de sesión de Spotify: no hay credenciales en caché que eliminar.",
    ),
    dynamic_translation(
        "Spotify logout failed: {}",
        "Échec de la déconnexion Spotify : {}",
        "Spotify-Abmeldung fehlgeschlagen: {}",
        "Error al cerrar sesión de Spotify: {}",
    ),
    dynamic_translation(
        "Could not determine the Spotify credential cache directory.",
        "Impossible de déterminer le dossier de cache des identifiants Spotify.",
        "Das Verzeichnis für die Spotify-Anmeldedaten konnte nicht bestimmt werden.",
        "No se pudo determinar el directorio de caché de credenciales de Spotify.",
    ),
    dynamic_translation(
        "Tidal login complete — tokens saved to '{}'.",
        "Connexion Tidal terminée — jetons enregistrés dans « {} ».",
        "Tidal-Anmeldung abgeschlossen — Token in „{}“ gespeichert.",
        "Inicio de sesión de Tidal completo — tokens guardados en «{}».",
    ),
    dynamic_translation(
        "Spotify login complete — credentials cached.",
        "Connexion Spotify terminée — identifiants mis en cache.",
        "Spotify-Anmeldung abgeschlossen — Anmeldedaten zwischengespeichert.",
        "Inicio de sesión de Spotify completo — credenciales guardadas en caché.",
    ),
    dynamic_translation(
        " Up/Down=Navigate  Enter=Edit field  s/Tab=Save  Esc=Cancel",
        " Haut/Bas=Naviguer  Entrée=Modifier le champ  s/Tab=Enregistrer  Échap=Annuler",
        " Hoch/Runter=Navigieren  Eingabe=Feld bearbeiten  s/Tab=Speichern  Esc=Abbrechen",
        " Arriba/Abajo=Navegar  Intro=Editar campo  s/Tab=Guardar  Esc=Cancelar",
    ),
    dynamic_translation(
        " Up/Down=Select type  Enter=Confirm  Esc=Cancel",
        " Haut/Bas=Choisir le type  Entrée=Confirmer  Échap=Annuler",
        " Hoch/Runter=Typ wählen  Eingabe=Bestätigen  Esc=Abbrechen",
        " Arriba/Abajo=Elegir tipo  Intro=Confirmar  Esc=Cancelar",
    ),
    dynamic_translation(
        " No sources configured. Press 'a' to add one.",
        " Aucune source configurée. Appuyez sur « a » pour en ajouter une.",
        " Keine Quellen konfiguriert. Mit „a“ eine hinzufügen.",
        " No hay fuentes configuradas. Pulse «a» para añadir una.",
    ),
    dynamic_translation("Name", "Nom", "Name", "Nombre"),
    dynamic_translation("Priority", "Priorité", "Priorität", "Prioridad"),
    dynamic_translation("Enabled", "Activé", "Aktiviert", "Activado"),
    dynamic_translation("untested", "non testée", "ungetestet", "sin probar"),
    dynamic_translation("testing...", "test…", "wird getestet…", "probando…"),
    dynamic_translation("connected", "connectée", "verbunden", "conectada"),
    dynamic_translation("error", "erreur", "Fehler", "error"),
    dynamic_translation("yes", "oui", "ja", "sí"),
    dynamic_translation("no", "non", "nein", "no"),
    dynamic_translation(
        " Library Sources ({}) ",
        " Sources de la bibliothèque ({}) ",
        " Mediatheksquellen ({}) ",
        " Fuentes de la biblioteca ({}) ",
    ),
    dynamic_translation(
        " New {} Source ",
        " Nouvelle source {} ",
        " Neue {}-Quelle ",
        " Nueva fuente {} ",
    ),
    dynamic_translation(
        " Edit: {} ",
        " Modifier : {} ",
        " Bearbeiten: {} ",
        " Editar: {} ",
    ),
    dynamic_translation(
        "Display Name",
        "Nom affiché",
        "Anzeigename",
        "Nombre visible",
    ),
    dynamic_translation("URL", "URL", "URL", "URL"),
    dynamic_translation("Username", "Nom d’utilisateur", "Benutzername", "Usuario"),
    dynamic_translation("Password", "Mot de passe", "Passwort", "Contraseña"),
    dynamic_translation(
        "Legacy Auth",
        "Ancienne authentification",
        "Legacy-Anmeldung",
        "Autenticación antigua",
    ),
    dynamic_translation("Host", "Hôte", "Host", "Host"),
    dynamic_translation("Port", "Port", "Port", "Puerto"),
    dynamic_translation(
        "Auth Mode",
        "Mode d’authentification",
        "Anmeldemodus",
        "Modo de autenticación",
    ),
    dynamic_translation(
        "HTTP Stream Port",
        "Port du flux HTTP",
        "HTTP-Stream-Port",
        "Puerto de flujo HTTP",
    ),
    dynamic_translation(
        "Location URL",
        "URL d’emplacement",
        "Standort-URL",
        "URL de ubicación",
    ),
    dynamic_translation(
        "Friendly Name",
        "Nom convivial",
        "Anzeigename",
        "Nombre descriptivo",
    ),
    dynamic_translation("Fingerprint", "Empreinte", "Fingerabdruck", "Huella"),
    dynamic_translation("API Token", "Jeton API", "API-Token", "Token de API"),
    dynamic_translation(
        "Access Token",
        "Jeton d’accès",
        "Zugriffstoken",
        "Token de acceso",
    ),
    dynamic_translation("Quality", "Qualité", "Qualität", "Calidad"),
    dynamic_translation("Country Code", "Code pays", "Ländercode", "Código de país"),
    dynamic_translation("Stream URL", "URL du flux", "Stream-URL", "URL del flujo"),
    dynamic_translation(
        "Station Name",
        "Nom de la station",
        "Sendername",
        "Nombre de emisora",
    ),
    dynamic_translation("None", "Aucun", "Keine", "Ninguno"),
    dynamic_translation("SSL", "SSL", "SSL", "SSL"),
    dynamic_translation("true", "vrai", "wahr", "verdadero"),
    dynamic_translation("false", "faux", "falsch", "falso"),
    dynamic_translation("Subsonic", "Subsonic", "Subsonic", "Subsonic"),
    dynamic_translation("MPD", "MPD", "MPD", "MPD"),
    dynamic_translation("DLNA", "DLNA", "DLNA", "DLNA"),
    dynamic_translation("Peer", "Pair", "Peer", "Par"),
    dynamic_translation("Peer (SotF)", "Pair (SotF)", "Peer (SotF)", "Par (SotF)"),
    dynamic_translation("Tidal", "Tidal", "Tidal", "Tidal"),
    dynamic_translation("Spotify", "Spotify", "Spotify", "Spotify"),
    dynamic_translation("Radio", "Radio", "Radio", "Radio"),
    dynamic_translation(
        " Select source type ",
        " Sélectionner le type de source ",
        " Quellentyp auswählen ",
        " Seleccionar tipo de fuente ",
    ),
    dynamic_translation(
        "  Connection diagnostic: {}:{}",
        "  Diagnostic de connexion : {}:{}",
        "  Verbindungsdiagnose: {}:{}",
        "  Diagnóstico de conexión: {}:{}",
    ),
    dynamic_translation(
        " Diagnostic ",
        " Diagnostic ",
        " Diagnose ",
        " Diagnóstico ",
    ),
    dynamic_translation(
        "Scanning Library",
        "Analyse de la bibliothèque",
        "Mediathek wird gescannt",
        "Escaneando la biblioteca",
    ),
    dynamic_translation(
        "Scanning directories for audio files...",
        "Recherche de fichiers audio dans les dossiers…",
        "Ordner werden nach Audiodateien durchsucht…",
        "Buscando archivos de audio en las carpetas…",
    ),
    dynamic_translation(
        "Tracks found: ",
        "Pistes trouvées : ",
        "Gefundene Titel: ",
        "Pistas encontradas: ",
    ),
    dynamic_translation(
        "Albums found: ",
        "Albums trouvés : ",
        "Gefundene Alben: ",
        "Álbumes encontrados: ",
    ),
    dynamic_translation(
        "Please wait...",
        "Veuillez patienter…",
        "Bitte warten…",
        "Espere…",
    ),
    dynamic_translation(
        "Database Maintenance",
        "Maintenance de la base de données",
        "Datenbankwartung",
        "Mantenimiento de la base de datos",
    ),
    dynamic_translation(
        "Checking database for missing files...",
        "Recherche des fichiers manquants dans la base de données…",
        "Datenbank wird auf fehlende Dateien geprüft…",
        "Comprobando archivos ausentes en la base de datos…",
    ),
    dynamic_translation(
        "Progress: ",
        "Progression : ",
        "Fortschritt: ",
        "Progreso: ",
    ),
    dynamic_translation(
        "ReplayGain Analysis",
        "Analyse ReplayGain",
        "ReplayGain-Analyse",
        "Análisis ReplayGain",
    ),
    dynamic_translation(
        "Analyzing tracks for ReplayGain...",
        "Analyse ReplayGain des pistes…",
        "Titel werden für ReplayGain analysiert…",
        "Analizando las pistas con ReplayGain…",
    ),
    dynamic_translation(
        "Succeeded: ",
        "Réussites : ",
        "Erfolgreich: ",
        "Correctas: ",
    ),
    dynamic_translation("  Failed: ", "  Échecs : ", "  Fehler: ", "  Fallidas: "),
    dynamic_translation(
        "a=add | s/S=scan | r/R=replay gain | b/B=bliss | w/W=waveform | d=delete | m=maintenance (uppercase=force)",
        "a=ajouter | s/S=analyser | r/R=ReplayGain | b/B=Bliss | w/W=forme d’onde | d=supprimer | m=maintenance (majuscule=forcer)",
        "a=hinzufügen | s/S=scannen | r/R=ReplayGain | b/B=Bliss | w/W=Wellenform | d=löschen | m=Wartung (Großbuchstabe=erzwingen)",
        "a=añadir | s/S=escanear | r/R=ReplayGain | b/B=Bliss | w/W=forma de onda | d=eliminar | m=mantenimiento (mayúscula=forzar)",
    ),
    dynamic_translation(
        "Path: {}█ (Tab to autocomplete)",
        "Chemin : {}█ (Tab pour compléter)",
        "Pfad: {}█ (Tab zum Vervollständigen)",
        "Ruta: {}█ (Tab para completar)",
    ),
    dynamic_translation(
        "Path: (Press 'a' to add directory)",
        "Chemin : (appuyez sur « a » pour ajouter un dossier)",
        "Pfad: („a“ drücken, um einen Ordner hinzuzufügen)",
        "Ruta: (pulse «a» para añadir una carpeta)",
    ),
    dynamic_translation(
        "Add Directory",
        "Ajouter un dossier",
        "Ordner hinzufügen",
        "Añadir carpeta",
    ),
    dynamic_translation("just now", "à l’instant", "gerade eben", "ahora mismo"),
    dynamic_translation("{} min ago", "il y a {} min", "vor {} Min.", "hace {} min"),
    dynamic_translation("{} hrs ago", "il y a {} h", "vor {} Std.", "hace {} h"),
    dynamic_translation(
        "{} days ago",
        "il y a {} jours",
        "vor {} Tagen",
        "hace {} días",
    ),
    dynamic_translation("never", "jamais", "nie", "nunca"),
    dynamic_translation(
        " [{} tracks, {} albums, {}]",
        " [{} pistes, {} albums, {}]",
        " [{} Titel, {} Alben, {}]",
        " [{} pistas, {} álbumes, {}]",
    ),
    dynamic_translation(
        " [{} tracks, {} albums]",
        " [{} pistes, {} albums]",
        " [{} Titel, {} Alben]",
        " [{} pistas, {} álbumes]",
    ),
    dynamic_translation("Directories", "Dossiers", "Ordner", "Carpetas"),
    dynamic_translation(" [paused]", " [en pause]", " [pausiert]", " [en pausa]"),
    dynamic_translation("Scanner", "Analyseur", "Scanner", "Escáner"),
    dynamic_translation("Status", "État", "Status", "Estado"),
    dynamic_translation("Fail", "Échec", "Fehler", "Fallos"),
    dynamic_translation("Total", "Total", "Gesamt", "Total"),
    dynamic_translation(
        "album {}/{}{}",
        "album {}/{}{}",
        "Album {}/{}{}",
        "álbum {}/{}{}",
    ),
    dynamic_translation("idle", "inactif", "inaktiv", "inactivo"),
    dynamic_translation("Waveform", "Forme d’onde", "Wellenform", "Forma de onda"),
    dynamic_translation("Library", "Bibliothèque", "Mediathek", "Biblioteca"),
    dynamic_translation(
        "{} tracks / {} albums{}",
        "{} pistes / {} albums{}",
        "{} Titel / {} Alben{}",
        "{} pistas / {} álbumes{}",
    ),
    dynamic_translation(
        "{} tracks / {} albums",
        "{} pistes / {} albums",
        "{} Titel / {} Alben",
        "{} pistas / {} álbumes",
    ),
    dynamic_translation(" Transport ", " Transport ", " Transport ", " Transporte "),
    dynamic_translation("Loading...", "Chargement…", "Laden…", "Cargando…"),
    dynamic_translation("Volume", "Volume", "Lautstärke", "Volumen"),
    dynamic_translation("Default", "Par défaut", "Standard", "Predeterminado"),
    dynamic_translation(
        "Output Device",
        "Périphérique de sortie",
        "Ausgabegerät",
        "Dispositivo de salida",
    ),
    dynamic_translation(
        "↑↓=Navigate  Enter=Open  n=New  r=Rename  d=Delete  p=Play  i=Import  e=Export  Esc=Back",
        "↑↓=Naviguer  Entrée=Ouvrir  n=Nouveau  r=Renommer  d=Supprimer  p=Lire  i=Importer  e=Exporter  Échap=Retour",
        "↑↓=Navigieren  Eingabe=Öffnen  n=Neu  r=Umbenennen  d=Löschen  p=Abspielen  i=Importieren  e=Exportieren  Esc=Zurück",
        "↑↓=Navegar  Intro=Abrir  n=Nueva  r=Renombrar  d=Eliminar  p=Reproducir  i=Importar  e=Exportar  Esc=Volver",
    ),
    dynamic_translation(
        "↑↓=Navigate  Enter=Play track  p=Play all  x=Remove  K/J=Move up/down  Esc=Back to list",
        "↑↓=Naviguer  Entrée=Lire la piste  p=Tout lire  x=Retirer  K/J=Déplacer  Échap=Retour à la liste",
        "↑↓=Navigieren  Eingabe=Titel abspielen  p=Alle abspielen  x=Entfernen  K/J=Verschieben  Esc=Zurück zur Liste",
        "↑↓=Navegar  Intro=Reproducir pista  p=Reproducir todo  x=Quitar  K/J=Mover  Esc=Volver a la lista",
    ),
    dynamic_translation(
        "Type playlist name  Enter=Create  Esc=Cancel",
        "Saisissez le nom  Entrée=Créer  Échap=Annuler",
        "Wiedergabelistenname eingeben  Eingabe=Erstellen  Esc=Abbrechen",
        "Escriba el nombre  Intro=Crear  Esc=Cancelar",
    ),
    dynamic_translation(
        "Type new name  Enter=Save  Esc=Cancel",
        "Saisissez le nouveau nom  Entrée=Enregistrer  Échap=Annuler",
        "Neuen Namen eingeben  Eingabe=Speichern  Esc=Abbrechen",
        "Escriba el nombre nuevo  Intro=Guardar  Esc=Cancelar",
    ),
    dynamic_translation(
        "y=Confirm delete  n/Esc=Cancel",
        "y=Confirmer la suppression  n/Échap=Annuler",
        "y=Löschen bestätigen  n/Esc=Abbrechen",
        "y=Confirmar eliminación  n/Esc=Cancelar",
    ),
    dynamic_translation(
        "New Playlist",
        "Nouvelle liste de lecture",
        "Neue Wiedergabeliste",
        "Nueva lista",
    ),
    dynamic_translation(
        "Rename Playlist",
        "Renommer la liste de lecture",
        "Wiedergabeliste umbenennen",
        "Renombrar lista",
    ),
    dynamic_translation("Confirm", "Confirmer", "Bestätigen", "Confirmar"),
    dynamic_translation(
        "Delete '{}'? (y/n)",
        "Supprimer « {} » ? (y/n)",
        "„{}“ löschen? (y/n)",
        "¿Eliminar «{}»? (y/n)",
    ),
    dynamic_translation(
        "n:new r:rename d:del Enter:open p:play i:import e:export",
        "n:nouveau r:renommer d:suppr Entrée:ouvrir p:lire i:importer e:exporter",
        "n:neu r:umbenennen d:löschen Eingabe:öffnen p:abspielen i:importieren e:exportieren",
        "n:nueva r:renombrar d:eliminar Intro:abrir p:reproducir i:importar e:exportar",
    ),
    dynamic_translation(
        "Playlists ({})",
        "Listes de lecture ({})",
        "Wiedergabelisten ({})",
        "Listas ({})",
    ),
    dynamic_translation(
        "{} ({} tracks)",
        "{} ({} pistes)",
        "{} ({} Titel)",
        "{} ({} pistas)",
    ),
    dynamic_translation(
        "Tracks (select a playlist)",
        "Pistes (sélectionnez une liste)",
        "Titel (Wiedergabeliste auswählen)",
        "Pistas (seleccione una lista)",
    ),
    dynamic_translation(
        "x:remove K/J:move Esc:back p:play",
        "x:retirer K/J:déplacer Échap:retour p:lire",
        "x:entfernen K/J:verschieben Esc:zurück p:abspielen",
        "x:quitar K/J:mover Esc:volver p:reproducir",
    ),
    dynamic_translation(
        " [Track {}/{}]",
        " [Piste {}/{}]",
        " [Titel {}/{}]",
        " [Pista {}/{}]",
    ),
    dynamic_translation(
        "Queue (empty)",
        "File d’attente (vide)",
        "Warteschlange (leer)",
        "Cola (vacía)",
    ),
    dynamic_translation(
        "Queue ({})",
        "File d’attente ({})",
        "Warteschlange ({})",
        "Cola ({})",
    ),
    dynamic_translation(
        "Album Art (none)",
        "Pochette (aucune)",
        "Albumcover (keines)",
        "Carátula (ninguna)",
    ),
    dynamic_translation(
        "Album Art ({}/{}) - [] to cycle",
        "Pochette ({}/{}) — [] pour parcourir",
        "Albumcover ({}/{}) – mit [] wechseln",
        "Carátula ({}/{}) — [] para cambiar",
    ),
    dynamic_translation("Album Art", "Pochette", "Albumcover", "Carátula"),
    dynamic_translation(
        "Failed to load image",
        "Échec du chargement de l’image",
        "Bild konnte nicht geladen werden",
        "No se pudo cargar la imagen",
    ),
    dynamic_translation(
        "No album art found",
        "Aucune pochette trouvée",
        "Kein Albumcover gefunden",
        "No se encontró ninguna carátula",
    ),
    dynamic_translation("Unknown", "Inconnu", "Unbekannt", "Desconocido"),
    dynamic_translation("Format: ", "Format : ", "Format: ", "Formato: "),
    dynamic_translation("Track RG: ", "RG piste : ", "Titel-RG: ", "RG de pista: "),
    dynamic_translation("Album RG: ", "RG album : ", "Album-RG: ", "RG de álbum: "),
    dynamic_translation(
        "not available",
        "indisponible",
        "nicht verfügbar",
        "no disponible",
    ),
    dynamic_translation(
        "No track playing",
        "Aucune piste en lecture",
        "Keine Wiedergabe",
        "No se está reproduciendo ninguna pista",
    ),
    dynamic_translation(
        "Search: {}█",
        "Recherche : {}█",
        "Suche: {}█",
        "Buscar: {}█",
    ),
    dynamic_translation("Search: {}", "Recherche : {}", "Suche: {}", "Buscar: {}"),
    dynamic_translation("Year", "Année", "Jahr", "Año"),
    dynamic_translation("Genre", "Genre", "Genre", "Género"),
    dynamic_translation("Artist", "Artiste", "Künstler", "Artista"),
    dynamic_translation("Album", "Album", "Album", "Álbum"),
    dynamic_translation("Tracks", "Pistes", "Titel", "Pistas"),
    dynamic_translation("Composer", "Compositeur", "Komponist", "Compositor"),
    dynamic_translation("Popularity", "Popularité", "Beliebtheit", "Popularidad"),
    dynamic_translation("All", "Tous", "Alle", "Todos"),
    dynamic_translation("Mono", "Mono", "Mono", "Mono"),
    dynamic_translation("Mixed", "Mixte", "Gemischt", "Mixto"),
    dynamic_translation(
        " | Available: {}",
        " | Disponibles : {}",
        " | Verfügbar: {}",
        " | Disponibles: {}",
    ),
    dynamic_translation(
        "Search Albums | Sort: ",
        "Rechercher des albums | Tri : ",
        "Alben suchen | Sortierung: ",
        "Buscar álbumes | Orden: ",
    ),
    dynamic_translation(" | Filter: ", " | Filtre : ", " | Filter: ", " | Filtro: "),
    dynamic_translation("Loss", "Perte", "Verlust", "Pérdida"),
    dynamic_translation(
        "Loss History  ({} iterations)",
        "Historique de perte  ({} itérations)",
        "Verlustverlauf  ({} Iterationen)",
        "Historial de pérdida  ({} iteraciones)",
    ),
    dynamic_translation("Iteration", "Itération", "Iteration", "Iteración"),
    dynamic_translation(
        "No curve data",
        "Aucune donnée de courbe",
        "Keine Kurvendaten",
        "No hay datos de curva",
    ),
    dynamic_translation(
        "Frequency Response",
        "Réponse en fréquence",
        "Frequenzgang",
        "Respuesta en frecuencia",
    ),
    dynamic_translation("Input", "Entrée", "Eingang", "Entrada"),
    dynamic_translation("Corrected", "Corrigée", "Korrigiert", "Corregida"),
    dynamic_translation("Filter", "Filtre", "Filter", "Filtro"),
    dynamic_translation(
        "Frequency Response (Gray=Input  Green=Corrected  Blue=Filter)",
        "Réponse en fréquence (gris=entrée  vert=corrigée  bleu=filtre)",
        "Frequenzgang (Grau=Eingang  Grün=Korrigiert  Blau=Filter)",
        "Respuesta en frecuencia (gris=entrada  verde=corregida  azul=filtro)",
    ),
    dynamic_translation(
        "↑↓=Navigate  Enter=Select  R=Reload  Esc=Back",
        "↑↓=Parcourir  Entrée=Sélectionner  R=Recharger  Échap=Retour",
        "↑↓=Navigieren  Eingabe=Auswählen  R=Neu laden  Esc=Zurück",
        "↑↓=Navegar  Intro=Seleccionar  R=Recargar  Esc=Volver",
    ),
    dynamic_translation(
        "↑↓=Navigate  Enter=Open  Esc=Back",
        "↑↓=Parcourir  Entrée=Ouvrir  Échap=Retour",
        "↑↓=Navigieren  Eingabe=Öffnen  Esc=Zurück",
        "↑↓=Navegar  Intro=Abrir  Esc=Volver",
    ),
    dynamic_translation(
        " [DEFAULT]",
        " [PAR DÉFAUT]",
        " [STANDARD]",
        " [PREDETERMINADO]",
    ),
    dynamic_translation(
        " Output Devices (none found) ",
        " Périphériques de sortie (aucun) ",
        " Ausgabegeräte (keine gefunden) ",
        " Dispositivos de salida (ninguno) ",
    ),
    dynamic_translation(
        " Output Devices ({}) ",
        " Périphériques de sortie ({}) ",
        " Ausgabegeräte ({}) ",
        " Dispositivos de salida ({}) ",
    ),
    dynamic_translation(
        " Cast Devices (scanning…) ",
        " Appareils Cast (recherche…) ",
        " Cast-Geräte (Suche…) ",
        " Dispositivos Cast (buscando…) ",
    ),
    dynamic_translation(
        " Cast Devices (none found — press R to scan) ",
        " Appareils Cast (aucun — R pour rechercher) ",
        " Cast-Geräte (keine — R zum Suchen) ",
        " Dispositivos Cast (ninguno — pulse R para buscar) ",
    ),
    dynamic_translation(
        " Cast Devices ({}) ",
        " Appareils Cast ({}) ",
        " Cast-Geräte ({}) ",
        " Dispositivos Cast ({}) ",
    ),
    dynamic_translation(
        "Directories        – Music library folders",
        "Dossiers           – Dossiers de la bibliothèque musicale",
        "Verzeichnisse      – Ordner der Musikbibliothek",
        "Directorios        – Carpetas de la biblioteca musical",
    ),
    dynamic_translation(
        "Recording          – Measure impulse responses",
        "Enregistrement     – Mesurer les réponses impulsionnelles",
        "Aufnahme           – Impulsantworten messen",
        "Grabación          – Medir respuestas al impulso",
    ),
    dynamic_translation(
        "Room EQ            – Optimize room correction filters",
        "EQ de salle        – Optimiser les filtres de correction",
        "Raum-EQ            – Raumkorrekturfilter optimieren",
        "EQ de sala         – Optimizar filtros de corrección",
    ),
    dynamic_translation(
        "Headphone EQ       – Target-curve EQ for headphones",
        "EQ casque          – Égalisation cible pour casque",
        "Kopfhörer-EQ       – Zielkurven-EQ für Kopfhörer",
        "EQ de auriculares  – EQ de curva objetivo",
    ),
    dynamic_translation(
        "Spinorama EQ       – Speaker EQ from spinorama data",
        "EQ Spinorama       – EQ d’enceinte depuis les données Spinorama",
        "Spinorama-EQ       – Lautsprecher-EQ aus Spinorama-Daten",
        "EQ Spinorama       – EQ de altavoz con datos Spinorama",
    ),
    dynamic_translation(
        "Library Sources    – Remote libraries (Subsonic, MPD, DLNA, Peer)",
        "Sources            – Bibliothèques distantes (Subsonic, MPD, DLNA, Pair)",
        "Bibliotheksquellen – Entfernte Bibliotheken (Subsonic, MPD, DLNA, Peer)",
        "Fuentes            – Bibliotecas remotas (Subsonic, MPD, DLNA, Par)",
    ),
    dynamic_translation(
        "Servers            – SOTF API, MPD and DLNA settings",
        "Serveurs           – Réglages API SOTF, MPD et DLNA",
        "Server             – SOTF-API-, MPD- und DLNA-Einstellungen",
        "Servidores         – Ajustes de API SOTF, MPD y DLNA",
    ),
    dynamic_translation(
        "Metadata Services  – MusicBrainz and tag provider settings",
        "Métadonnées        – Réglages MusicBrainz et fournisseur de tags",
        "Metadatendienste   – MusicBrainz- und Tag-Anbieter-Einstellungen",
        "Metadatos          – Ajustes de MusicBrainz y proveedor de etiquetas",
    ),
    dynamic_translation(
        " Directories ",
        " Dossiers ",
        " Verzeichnisse ",
        " Directorios ",
    ),
    dynamic_translation(
        " Recording ",
        " Enregistrement ",
        " Aufnahme ",
        " Grabación ",
    ),
    dynamic_translation(" Room EQ ", " EQ de salle ", " Raum-EQ ", " EQ de sala "),
    dynamic_translation(
        " Headphone EQ ",
        " EQ casque ",
        " Kopfhörer-EQ ",
        " EQ de auriculares ",
    ),
    dynamic_translation(
        " Spinorama EQ ",
        " EQ Spinorama ",
        " Spinorama-EQ ",
        " EQ Spinorama ",
    ),
    dynamic_translation(
        " Library Sources ",
        " Sources ",
        " Bibliotheksquellen ",
        " Fuentes ",
    ),
    dynamic_translation(" Servers ", " Serveurs ", " Server ", " Servidores "),
    dynamic_translation(
        " Metadata Services ",
        " Métadonnées ",
        " Metadatendienste ",
        " Metadatos ",
    ),
    dynamic_translation(
        "{} (Esc to close)",
        "{} (Échap pour fermer)",
        "{} (Esc zum Schließen)",
        "{} (Esc para cerrar)",
    ),
    dynamic_translation("Anonymous", "Anonyme", "Anonym", "Anónimo"),
    dynamic_translation(
        "Credentials saved",
        "Identifiants enregistrés",
        "Zugangsdaten gespeichert",
        "Credenciales guardadas",
    ),
    dynamic_translation(
        "Anonymous search enabled",
        "Recherche anonyme activée",
        "Anonyme Suche aktiviert",
        "Búsqueda anónima activada",
    ),
    dynamic_translation(
        "Endpoint: {}",
        "Point d’accès : {}",
        "Endpunkt: {}",
        "Punto de acceso: {}",
    ),
    dynamic_translation("Account: {}", "Compte : {}", "Konto: {}", "Cuenta: {}"),
    dynamic_translation("Status: {}", "État : {}", "Status: {}", "Estado: {}"),
    dynamic_translation(
        "User-Agent: {}",
        "Agent utilisateur : {}",
        "User-Agent: {}",
        "Agente de usuario: {}",
    ),
    dynamic_translation("File", "Fichier", "Datei", "Archivo"),
    dynamic_translation("Select", "Sélectionner", "Auswählen", "Seleccionar"),
    dynamic_translation("Load Data", "Charger", "Daten laden", "Cargar datos"),
    dynamic_translation("Delay", "Délai", "Verzögerung", "Retardo"),
    dynamic_translation("Process", "Processus", "Ablauf", "Proceso"),
    dynamic_translation("Config", "Configuration", "Konfiguration", "Configuración"),
    dynamic_translation("Configure", "Configurer", "Konfigurieren", "Configurar"),
    dynamic_translation("Optimize", "Optimiser", "Optimieren", "Optimizar"),
    dynamic_translation("Results", "Résultats", "Ergebnisse", "Resultados"),
    dynamic_translation("Review", "Examiner", "Prüfen", "Revisar"),
    dynamic_translation("Export", "Exporter", "Exportieren", "Exportar"),
    dynamic_translation(
        "Update Plugin",
        "Mettre à jour le module",
        "Plugin aktualisieren",
        "Actualizar complemento",
    ),
    dynamic_translation("SPL Cal", "Cal. SPL", "SPL-Kal.", "Cal. SPL"),
    dynamic_translation("Capture", "Capturer", "Aufnehmen", "Capturar"),
    dynamic_translation("Probe", "Sonde", "Sonde", "Sonda"),
    dynamic_translation(
        "Bass Anchor",
        "Ancrage des graves",
        "Bassanker",
        "Anclaje de graves",
    ),
    dynamic_translation("Evaluate", "Évaluer", "Auswerten", "Evaluar"),
    dynamic_translation("Save", "Enregistrer", "Speichern", "Guardar"),
    dynamic_translation("Spinorama", "Spinorama", "Spinorama", "Spinorama"),
    dynamic_translation(
        "Load from File",
        "Charger depuis un fichier",
        "Aus Datei laden",
        "Cargar desde archivo",
    ),
    dynamic_translation(
        "Download from spinorama.org",
        "Télécharger depuis spinorama.org",
        "Von spinorama.org laden",
        "Descargar de spinorama.org",
    ),
    dynamic_translation(
        "── Devices ──",
        "── Appareils ──",
        "── Geräte ──",
        "── Dispositivos ──",
    ),
    dynamic_translation(
        "Playback Device",
        "Périphérique de lecture",
        "Wiedergabegerät",
        "Dispositivo de reproducción",
    ),
    dynamic_translation(
        "Recording Device",
        "Périphérique d’enregistrement",
        "Aufnahmegerät",
        "Dispositivo de grabación",
    ),
    dynamic_translation(
        "Speaker Config",
        "Configuration des enceintes",
        "Lautsprecherkonfiguration",
        "Configuración de altavoces",
    ),
    dynamic_translation(
        "── Signal ──",
        "── Signal ──",
        "── Signal ──",
        "── Señal ──",
    ),
    dynamic_translation(
        "Signal Type",
        "Type de signal",
        "Signaltyp",
        "Tipo de señal",
    ),
    dynamic_translation("Duration (s)", "Durée (s)", "Dauer (s)", "Duración (s)"),
    dynamic_translation("Level (dB)", "Niveau (dB)", "Pegel (dB)", "Nivel (dB)"),
    dynamic_translation(
        "Sweep Start (Hz)",
        "Début du balayage (Hz)",
        "Sweep-Start (Hz)",
        "Inicio del barrido (Hz)",
    ),
    dynamic_translation(
        "Sweep End (Hz)",
        "Fin du balayage (Hz)",
        "Sweep-Ende (Hz)",
        "Fin del barrido (Hz)",
    ),
    dynamic_translation("── Paths ──", "── Chemins ──", "── Pfade ──", "── Rutas ──"),
    dynamic_translation(
        "Output Directory",
        "Dossier de sortie",
        "Ausgabeverzeichnis",
        "Directorio de salida",
    ),
    dynamic_translation(
        "── Recording Channels ──",
        "── Canaux d’enregistrement ──",
        "── Aufnahmekanäle ──",
        "── Canales de grabación ──",
    ),
    dynamic_translation(
        "Num Channels",
        "Nombre de canaux",
        "Kanalanzahl",
        "Número de canales",
    ),
    dynamic_translation("CTC Matrix", "Matrice CTC", "CTC-Matrix", "Matriz CTC"),
    dynamic_translation(
        "Loopback Input",
        "Entrée de bouclage",
        "Loopback-Eingang",
        "Entrada de bucle",
    ),
    dynamic_translation(
        "Mic Cal Ch{}",
        "Cal. micro canal {}",
        "Mikro-Kal. Kanal {}",
        "Cal. micro canal {}",
    ),
    dynamic_translation(
        "Mic Calibration",
        "Étalonnage du micro",
        "Mikrofonkalibrierung",
        "Calibración del micrófono",
    ),
    dynamic_translation(
        "Ch{} input",
        "Entrée canal {}",
        "Kanal-{}-Eingang",
        "Entrada canal {}",
    ),
    dynamic_translation(
        "(no devices)",
        "(aucun appareil)",
        "(keine Geräte)",
        "(sin dispositivos)",
    ),
    dynamic_translation("(select)", "(sélectionner)", "(auswählen)", "(seleccionar)"),
    dynamic_translation("<none>", "<aucun>", "<keine>", "<ninguno>"),
    dynamic_translation(
        "<not set>",
        "<non défini>",
        "<nicht gesetzt>",
        "<sin definir>",
    ),
    dynamic_translation("  Channels:", "  Canaux :", "  Kanäle:", "  Canales:"),
    dynamic_translation(
        "    {} → ch {}",
        "    {} → canal {}",
        "    {} → Kanal {}",
        "    {} → canal {}",
    ),
    dynamic_translation(
        " Type value, Enter=confirm  Esc=cancel",
        " Saisir la valeur, Entrée=confirmer  Échap=annuler",
        " Wert eingeben, Eingabe=Bestätigen  Esc=Abbrechen",
        " Escriba el valor, Intro=confirmar  Esc=cancelar",
    ),
    dynamic_translation(
        " Type path, Tab=complete  Enter=confirm  F2=browse  Esc=cancel",
        " Saisir le chemin, Tab=compléter  Entrée=confirmer  F2=parcourir  Échap=annuler",
        " Pfad eingeben, Tab=Vervollständigen  Eingabe=Bestätigen  F2=Durchsuchen  Esc=Abbrechen",
        " Escriba la ruta, Tab=completar  Intro=confirmar  F2=examinar  Esc=cancelar",
    ),
    dynamic_translation(
        " Up/Down=navigate  Left/Right=adjust  Enter=edit value/path  Tab=next field",
        " Haut/Bas=parcourir  Gauche/Droite=régler  Entrée=modifier valeur/chemin  Tab=champ suivant",
        " Hoch/Runter=Navigieren  Links/Rechts=Anpassen  Eingabe=Wert/Pfad bearbeiten  Tab=Nächstes Feld",
        " Arriba/Abajo=navegar  Izq./Der.=ajustar  Intro=editar valor/ruta  Tab=campo siguiente",
    ),
    dynamic_translation(
        "Ready to record. Select a channel and press Enter.",
        "Prêt à enregistrer. Sélectionnez un canal puis appuyez sur Entrée.",
        "Aufnahmebereit. Kanal auswählen und Eingabe drücken.",
        "Listo para grabar. Seleccione un canal y pulse Intro.",
    ),
    dynamic_translation(" Channel: {}", " Canal : {}", " Kanal: {}", " Canal: {}"),
    dynamic_translation(
        " Frequency points: {}",
        " Points de fréquence : {}",
        " Frequenzpunkte: {}",
        " Puntos de frecuencia: {}",
    ),
    dynamic_translation(
        " Avg THD: {}%",
        " THD moyenne : {} %",
        " Mittlere THD: {} %",
        " THD media: {} %",
    ),
    dynamic_translation(
        " Avg RT60: {} ms",
        " RT60 moyen : {} ms",
        " Mittlere RT60: {} ms",
        " RT60 medio: {} ms",
    ),
    dynamic_translation(
        "Session Name (editing)",
        "Nom de session (édition)",
        "Sitzungsname (Bearbeitung)",
        "Nombre de sesión (editando)",
    ),
    dynamic_translation(
        "Session Name",
        "Nom de session",
        "Sitzungsname",
        "Nombre de sesión",
    ),
    dynamic_translation(
        "type session name",
        "saisir le nom de session",
        "Sitzungsnamen eingeben",
        "escriba el nombre de sesión",
    ),
    dynamic_translation(
        "Room Dimensions ({})",
        "Dimensions de la salle ({})",
        "Raumabmessungen ({})",
        "Dimensiones de la sala ({})",
    ),
    dynamic_translation(" W: {}", " L : {}", " B: {}", " A: {}"),
    dynamic_translation(" D: {}", " P : {}", " T: {}", " P: {}"),
    dynamic_translation(" H: {}", " H : {}", " H: {}", " A: {}"),
    dynamic_translation("[Metric]", "[Métrique]", "[Metrisch]", "[Métrico]"),
    dynamic_translation("[Imperial]", "[Impérial]", "[Imperial]", "[Imperial]"),
    dynamic_translation(
        "Setup Description (editing)",
        "Description de l’installation (édition)",
        "Aufbaubeschreibung (Bearbeitung)",
        "Descripción de la instalación (editando)",
    ),
    dynamic_translation(
        "Setup Description",
        "Description de l’installation",
        "Aufbaubeschreibung",
        "Descripción de la instalación",
    ),
    dynamic_translation(
        "describe treatment, seating, equipment",
        "décrire le traitement, l’assise et l’équipement",
        "Akustik, Sitzplätze und Geräte beschreiben",
        "describa tratamiento, asientos y equipo",
    ),
    dynamic_translation(
        "Speakers per Channel  (catalog loading…)",
        "Enceintes par canal  (chargement du catalogue…)",
        "Lautsprecher pro Kanal  (Katalog wird geladen…)",
        "Altavoces por canal  (cargando catálogo…)",
    ),
    dynamic_translation(
        "Speakers per Channel",
        "Enceintes par canal",
        "Lautsprecher pro Kanal",
        "Altavoces por canal",
    ),
    dynamic_translation("<empty>", "<vide>", "<leer>", "<vacío>"),
    dynamic_translation(
        " Loading catalog…",
        " Chargement du catalogue…",
        " Katalog wird geladen…",
        " Cargando catálogo…",
    ),
    dynamic_translation(
        " No matches — free-form text is saved as-is",
        " Aucun résultat — le texte libre sera conservé",
        " Keine Treffer — Freitext wird unverändert gespeichert",
        " Sin coincidencias — el texto libre se guardará tal cual",
    ),
    dynamic_translation(" ▸ {}", " ▸ {}", " ▸ {}", " ▸ {}"),
    dynamic_translation(
        " {} channels ready. Output: {}",
        " {} canaux prêts. Sortie : {}",
        " {} Kanäle bereit. Ausgabe: {}",
        " {} canales listos. Salida: {}",
    ),
    dynamic_translation(
        " Tab=next field  ↑↓=nav  Enter=edit  u=unit  Ctrl+S=save",
        " Tab=champ suivant  ↑↓=parcourir  Entrée=modifier  u=unité  Ctrl+S=enregistrer",
        " Tab=Nächstes Feld  ↑↓=Navigation  Eingabe=Bearbeiten  u=Einheit  Strg+S=Speichern",
        " Tab=campo siguiente  ↑↓=navegar  Intro=editar  u=unidad  Ctrl+S=guardar",
    ),
    dynamic_translation("running...", "en cours...", "läuft...", "en curso..."),
    dynamic_translation("running…", "en cours…", "läuft…", "en curso…"),
    dynamic_translation("done", "terminé", "fertig", "hecho"),
    dynamic_translation("failed", "échec", "fehlgeschlagen", "falló"),
    dynamic_translation(
        "press r or Enter",
        "appuyer sur r ou Entrée",
        "r oder Eingabe drücken",
        "pulse r o Intro",
    ),
    dynamic_translation(
        "Idle — press `r` to capture tone-burst delays",
        "Inactif — appuyez sur `r` pour capturer les délais",
        "Bereit — mit `r` Tonimpulsverzögerungen erfassen",
        "Inactivo — pulse `r` para capturar los retardos",
    ),
    dynamic_translation(
        "Running... {}",
        "Capture... {}",
        "Erfassung... {}",
        "Capturando... {}",
    ),
    dynamic_translation(
        "Complete — detected {} channel(s)",
        "Terminé — {} canal(aux) détecté(s)",
        "Fertig — {} Kanal/Kanäle erkannt",
        "Completado — {} canal(es) detectado(s)",
    ),
    dynamic_translation(
        "Failed: {}",
        "Échec : {}",
        "Fehlgeschlagen: {}",
        "Error: {}",
    ),
    dynamic_translation("Error: {}", "Erreur : {}", "Fehler: {}", "Error: {}"),
    dynamic_translation(
        " Tab=next field  ←→=adjust  r=run  Tab=evaluate",
        " Tab=champ suivant  ←→=régler  r=lancer  Tab=évaluer",
        " Tab=Nächstes Feld  ←→=Anpassen  r=Start  Tab=Auswerten",
        " Tab=campo siguiente  ←→=ajustar  r=ejecutar  Tab=evaluar",
    ),
    dynamic_translation(
        "Plays a steady-state bass tone per channel so GD-Opt v2 can lock-in the",
        "Joue un son grave stationnaire par canal afin que GD-Opt v2 puisse verrouiller",
        "Spielt je Kanal einen stationären Basston, damit GD-Opt v2",
        "Reproduce un tono grave estable por canal para que GD-Opt v2 pueda fijar",
    ),
    dynamic_translation(
        "first bass bin of the sweep-derived phase. Optional — skip with Tab.",
        "le premier point grave de la phase issue du balayage. Facultatif — Tab pour ignorer.",
        "den ersten Bass-Bin der Sweep-Phase fixiert. Optional — mit Tab überspringen.",
        "el primer bin grave de la fase del barrido. Opcional — omita con Tab.",
    ),
    dynamic_translation(
        " Tone: {} Hz × {} s ({} sub-windows) • silence {} ms • mic ch {}{}",
        " Ton : {} Hz × {} s ({} sous-fenêtres) • silence {} ms • canal micro {}{}",
        " Ton: {} Hz × {} s ({} Teilfenster) • Stille {} ms • Mikro-Kanal {}{}",
        " Tono: {} Hz × {} s ({} subventanas) • silencio {} ms • canal de micro {}{}",
    ),
    dynamic_translation(
        " • loopback ref ch {}",
        " • canal de référence bouclé {}",
        " • Loopback-Referenzkanal {}",
        " • canal de referencia de bucle {}",
    ),
    dynamic_translation(
        "Idle — optional step ({} ms / channel).",
        "Inactif — étape facultative ({} ms/canal).",
        "Bereit — optionaler Schritt ({} ms/Kanal).",
        "Inactivo — paso opcional ({} ms/canal).",
    ),
    dynamic_translation(
        "Capturing bass anchor…",
        "Capture de l’ancrage des graves…",
        "Bassanker wird erfasst…",
        "Capturando anclaje de graves…",
    ),
    dynamic_translation(
        "Complete — {} channel(s) analysed",
        "Terminé — {} canal(aux) analysé(s)",
        "Fertig — {} Kanal/Kanäle analysiert",
        "Completado — {} canal(es) analizado(s)",
    ),
    dynamic_translation("OK", "OK", "OK", "OK"),
    dynamic_translation(
        "⚠ unreliable (>20°)",
        "⚠ peu fiable (>20°)",
        "⚠ unzuverlässig (>20°)",
        "⚠ poco fiable (>20°)",
    ),
    dynamic_translation(
        "Results (sample rate {} Hz)",
        "Résultats (fréquence {} Hz)",
        "Ergebnisse (Abtastrate {} Hz)",
        "Resultados (frecuencia {} Hz)",
    ),
    dynamic_translation(
        " Tab/BackTab=switch step  (display-only — optional)",
        " Tab/RetourTab=changer d’étape  (affichage seul — facultatif)",
        " Tab/Umschalt+Tab=Schritt wechseln  (nur Anzeige — optional)",
        " Tab/Mayús+Tab=cambiar paso  (solo lectura — opcional)",
    ),
    dynamic_translation(
        "[ Cancel tone ]",
        "[ Annuler le son ]",
        "[ Ton abbrechen ]",
        "[ Cancelar tono ]",
    ),
    dynamic_translation(
        "[ Play calibration tone ]",
        "[ Jouer le son d’étalonnage ]",
        "[ Kalibrierton abspielen ]",
        "[ Reproducir tono de calibración ]",
    ),
    dynamic_translation(
        "[ Playing… ]",
        "[ Lecture… ]",
        "[ Wiedergabe… ]",
        "[ Reproduciendo… ]",
    ),
    dynamic_translation(
        "[ Re-play tone ]",
        "[ Rejouer le son ]",
        "[ Ton erneut abspielen ]",
        "[ Repetir tono ]",
    ),
    dynamic_translation(
        "type meter reading",
        "saisir la valeur du sonomètre",
        "Messwert eingeben",
        "escriba la lectura del medidor",
    ),
    dynamic_translation(
        "Ready — {} Hz @ amp {} for {}s on ch {}",
        "Prêt — {} Hz, amplitude {}, {} s sur le canal {}",
        "Bereit — {} Hz bei Amplitude {} für {} s auf Kanal {}",
        "Listo — {} Hz, amplitud {}, {} s en canal {}",
    ),
    dynamic_translation(
        "Tone playing — read your SPL meter now…",
        "Son en cours — lisez maintenant votre sonomètre…",
        "Ton läuft — jetzt SPL-Messgerät ablesen…",
        "Tono en curso — lea ahora el medidor SPL…",
    ),
    dynamic_translation(
        "Tone captured — peak {}, RMS {}. Enter the dBSPL your meter showed.",
        "Son capturé — crête {}, RMS {}. Saisissez le dBSPL affiché.",
        "Ton erfasst — Spitze {}, RMS {}. Angezeigten dBSPL-Wert eingeben.",
        "Tono capturado — pico {}, RMS {}. Introduzca los dBSPL indicados.",
    ),
    dynamic_translation("Complete", "Terminé", "Fertig", "Completado"),
    dynamic_translation(
        " Sample rate {} Hz  •  peak {}  •  RMS {}  •  ref {} Hz  •  out ch {}",
        " Fréquence {} Hz  •  crête {}  •  RMS {}  •  réf. {} Hz  •  canal sortie {}",
        " Abtastrate {} Hz  •  Spitze {}  •  RMS {}  •  Ref. {} Hz  •  Ausgangskanal {}",
        " Frecuencia {} Hz  •  pico {}  •  RMS {}  •  ref. {} Hz  •  canal de salida {}",
    ),
    dynamic_translation(
        " Reported dBSPL: {}",
        " dBSPL indiqué : {}",
        " Gemeldeter dBSPL: {}",
        " dBSPL indicado: {}",
    ),
    dynamic_translation(
        " Reported dBSPL: (move to the field below and type your meter reading)",
        " dBSPL indiqué : (allez au champ ci-dessous et saisissez la mesure)",
        " Gemeldeter dBSPL: (zum Feld unten wechseln und Messwert eingeben)",
        " dBSPL indicado: (vaya al campo inferior y escriba la lectura)",
    ),
    dynamic_translation(
        " → spl_offset_db = {}  (will be stored on Save)",
        " → spl_offset_db = {}  (sera enregistré à l’étape Enregistrer)",
        " → spl_offset_db = {}  (wird beim Speichern gesichert)",
        " → spl_offset_db = {}  (se guardará al pulsar Guardar)",
    ),
    dynamic_translation(
        " No tone captured yet — press r (or Enter on the Run row) to play the reference tone.",
        " Aucun son capturé — appuyez sur r (ou Entrée sur la ligne Lancer) pour jouer le son de référence.",
        " Noch kein Ton erfasst — r (oder Eingabe in der Startzeile) spielt den Referenzton.",
        " Aún no hay tono capturado — pulse r (o Intro en la fila Ejecutar) para reproducirlo.",
    ),
    dynamic_translation(
        " r/Enter=cancel  Tab/BackTab=field  ←→=adjust",
        " r/Entrée=annuler  Tab/RetourTab=champ  ←→=régler",
        " r/Eingabe=Abbrechen  Tab/Umschalt+Tab=Feld  ←→=Anpassen",
        " r/Intro=cancelar  Tab/Mayús+Tab=campo  ←→=ajustar",
    ),
    dynamic_translation(
        " Tab=next field  ←→=adjust  Enter=edit/run  r=run  Tab=Capture",
        " Tab=champ suivant  ←→=régler  Entrée=modifier/lancer  r=lancer  Tab=Capturer",
        " Tab=Nächstes Feld  ←→=Anpassen  Eingabe=Bearbeiten/Start  r=Start  Tab=Aufnahme",
        " Tab=campo siguiente  ←→=ajustar  Intro=editar/ejecutar  r=ejecutar  Tab=Capturar",
    ),
    dynamic_translation(
        "Search (loading...)",
        "Recherche (chargement...)",
        "Suche (Laden...)",
        "Buscar (cargando...)",
    ),
    dynamic_translation(
        "Search (editing)",
        "Recherche (édition)",
        "Suche (Bearbeitung)",
        "Buscar (editando)",
    ),
    dynamic_translation(
        "Search (downloading...)",
        "Recherche (téléchargement...)",
        "Suche (Download...)",
        "Buscar (descargando...)",
    ),
    dynamic_translation(
        "Search headphones (Enter to edit)",
        "Rechercher un casque (Entrée pour modifier)",
        "Kopfhörer suchen (Eingabe zum Bearbeiten)",
        "Buscar auriculares (Intro para editar)",
    ),
    dynamic_translation(
        "Selected: {}",
        "Sélectionné : {}",
        "Ausgewählt: {}",
        "Seleccionado: {}",
    ),
    dynamic_translation(
        "<type to search>",
        "<saisir pour rechercher>",
        "<Suchtext eingeben>",
        "<escriba para buscar>",
    ),
    dynamic_translation(
        "{} matches",
        "{} résultats",
        "{} Treffer",
        "{} coincidencias",
    ),
    dynamic_translation(
        "Downloaded: {}",
        "Téléchargé : {}",
        "Heruntergeladen: {}",
        "Descargado: {}",
    ),
    dynamic_translation(
        "Measurement CSV (editing)",
        "CSV de mesure (édition)",
        "Mess-CSV (Bearbeitung)",
        "CSV de medición (editando)",
    ),
    dynamic_translation(
        "Measurement CSV",
        "CSV de mesure",
        "Mess-CSV",
        "CSV de medición",
    ),
    dynamic_translation(
        "<type path or paste>",
        "<saisir ou coller le chemin>",
        "<Pfad eingeben oder einfügen>",
        "<escriba o pegue la ruta>",
    ),
    dynamic_translation(
        "Custom Target (editing)",
        "Cible personnalisée (édition)",
        "Benutzerziel (Bearbeitung)",
        "Objetivo personalizado (editando)",
    ),
    dynamic_translation(
        "Custom Target CSV",
        "CSV de cible personnalisée",
        "Benutzerziel-CSV",
        "CSV de objetivo personalizado",
    ),
    dynamic_translation(
        "<type path>",
        "<saisir le chemin>",
        "<Pfad eingeben>",
        "<escriba la ruta>",
    ),
    dynamic_translation(
        " Up/Down=select field  Enter=edit  Left/Right=toggle/cycle  Tab=next step",
        " Haut/Bas=sélectionner  Entrée=modifier  Gauche/Droite=basculer/parcourir  Tab=étape suivante",
        " Hoch/Runter=Feld  Eingabe=Bearbeiten  Links/Rechts=Umschalten  Tab=Nächster Schritt",
        " Arriba/Abajo=campo  Intro=editar  Izq./Der.=alternar  Tab=paso siguiente",
    ),
    dynamic_translation("Simple", "Simple", "Einfach", "Simple"),
    dynamic_translation("Customize", "Personnaliser", "Anpassen", "Personalizar"),
    dynamic_translation(
        "All Parameters",
        "Tous les paramètres",
        "Alle Parameter",
        "Todos los parámetros",
    ),
    dynamic_translation(
        "  [{}]  Tab=cycle mode",
        "  [{}]  Tab=changer de mode",
        "  [{}]  Tab=Modus wechseln",
        "  [{}]  Tab=cambiar modo",
    ),
    dynamic_translation(
        "── Preset ──",
        "── Préréglage ──",
        "── Preset ──",
        "── Preajuste ──",
    ),
    dynamic_translation("Preset", "Préréglage", "Preset", "Preajuste"),
    dynamic_translation(
        "Quick Fix",
        "Correction rapide",
        "Schnellkorrektur",
        "Corrección rápida",
    ),
    dynamic_translation("Balanced", "Équilibré", "Ausgewogen", "Equilibrado"),
    dynamic_translation(
        "Maximum Quality",
        "Qualité maximale",
        "Maximale Qualität",
        "Calidad máxima",
    ),
    dynamic_translation(
        "Custom",
        "Personnalisé",
        "Benutzerdefiniert",
        "Personalizado",
    ),
    dynamic_translation(
        "Fast correction with 5 filters. Good for a quick improvement.",
        "Correction rapide avec 5 filtres, idéale pour une amélioration immédiate.",
        "Schnelle Korrektur mit 5 Filtern für eine rasche Verbesserung.",
        "Corrección rápida con 5 filtros para una mejora inmediata.",
    ),
    dynamic_translation(
        "Good balance of quality and speed. Recommended for most headphones.",
        "Bon équilibre entre qualité et vitesse, recommandé pour la plupart des casques.",
        "Gutes Verhältnis von Qualität und Geschwindigkeit; für die meisten Kopfhörer empfohlen.",
        "Buen equilibrio entre calidad y velocidad; recomendado para la mayoría de auriculares.",
    ),
    dynamic_translation(
        "Best possible correction with shelves. Slower optimization.",
        "Meilleure correction possible avec des filtres en plateau ; optimisation plus lente.",
        "Bestmögliche Korrektur mit Shelving-Filtern; langsamere Optimierung.",
        "La mejor corrección posible con filtros shelf; optimización más lenta.",
    ),
    dynamic_translation(
        "Full control over all optimization parameters.",
        "Contrôle complet de tous les paramètres d’optimisation.",
        "Volle Kontrolle über alle Optimierungsparameter.",
        "Control total de todos los parámetros de optimización.",
    ),
    dynamic_translation(
        "── Filter Design ──",
        "── Conception des filtres ──",
        "── Filterentwurf ──",
        "── Diseño de filtros ──",
    ),
    dynamic_translation(
        "── Filters ──",
        "── Filtres ──",
        "── Filter ──",
        "── Filtros ──",
    ),
    dynamic_translation(
        "── Goal ──",
        "── Objectif ──",
        "── Ziel ──",
        "── Objetivo ──",
    ),
    dynamic_translation(
        "── Basic ──",
        "── Base ──",
        "── Grundlagen ──",
        "── Básico ──",
    ),
    dynamic_translation(
        "── Loss ──",
        "── Perte ──",
        "── Verlust ──",
        "── Pérdida ──",
    ),
    dynamic_translation(
        "── Optimization ──",
        "── Optimisation ──",
        "── Optimierung ──",
        "── Optimización ──",
    ),
    dynamic_translation(
        "── Refinement ──",
        "── Affinement ──",
        "── Verfeinerung ──",
        "── Refinamiento ──",
    ),
    dynamic_translation(
        "── Smoothing ──",
        "── Lissage ──",
        "── Glättung ──",
        "── Suavizado ──",
    ),
    dynamic_translation("── Mode ──", "── Mode ──", "── Modus ──", "── Modo ──"),
    dynamic_translation(
        "── Target Response ──",
        "── Réponse cible ──",
        "── Zielantwort ──",
        "── Respuesta objetivo ──",
    ),
    dynamic_translation(
        "── Excursion ──",
        "── Excursion ──",
        "── Auslenkung ──",
        "── Excursión ──",
    ),
    dynamic_translation(
        "── Schroeder Split ──",
        "── Séparation de Schroeder ──",
        "── Schroeder-Trennung ──",
        "── División de Schroeder ──",
    ),
    dynamic_translation("── Phase ──", "── Phase ──", "── Phase ──", "── Fase ──"),
    dynamic_translation(
        "── Constraints ──",
        "── Contraintes ──",
        "── Beschränkungen ──",
        "── Restricciones ──",
    ),
    dynamic_translation(
        "── Convergence ──",
        "── Convergence ──",
        "── Konvergenz ──",
        "── Convergencia ──",
    ),
    dynamic_translation("Filters (n)", "Filtres (n)", "Filter (n)", "Filtros (n)"),
    dynamic_translation(
        "Filter Type",
        "Type de filtre",
        "Filtertyp",
        "Tipo de filtro",
    ),
    dynamic_translation(
        "Min Freq (Hz)",
        "Fréq. min. (Hz)",
        "Min. Frequenz (Hz)",
        "Frec. mín. (Hz)",
    ),
    dynamic_translation(
        "Max Freq (Hz)",
        "Fréq. max. (Hz)",
        "Max. Frequenz (Hz)",
        "Frec. máx. (Hz)",
    ),
    dynamic_translation("Min dB", "dB min.", "Min. dB", "dB mín."),
    dynamic_translation("Max dB", "dB max.", "Max. dB", "dB máx."),
    dynamic_translation("Min Q", "Q min.", "Min. Q", "Q mín."),
    dynamic_translation("Max Q", "Q max.", "Max. Q", "Q máx."),
    dynamic_translation(
        "Loss Function",
        "Fonction de perte",
        "Verlustfunktion",
        "Función de pérdida",
    ),
    dynamic_translation("PEQ Model", "Modèle PEQ", "PEQ-Modell", "Modelo PEQ"),
    dynamic_translation("Algorithm", "Algorithme", "Algorithmus", "Algoritmo"),
    dynamic_translation(
        "Max Iter",
        "Itérations max.",
        "Max. Iterationen",
        "Iteraciones máx.",
    ),
    dynamic_translation("Population", "Population", "Population", "Población"),
    dynamic_translation("Strategy", "Stratégie", "Strategie", "Estrategia"),
    dynamic_translation(
        "DE F (mutation)",
        "DE F (mutation)",
        "DE F (Mutation)",
        "DE F (mutación)",
    ),
    dynamic_translation(
        "DE CR (crossover)",
        "DE CR (croisement)",
        "DE CR (Kreuzung)",
        "DE CR (cruce)",
    ),
    dynamic_translation("Refine", "Affiner", "Verfeinern", "Refinar"),
    dynamic_translation(
        "Local Algo",
        "Algo local",
        "Lokaler Algorithmus",
        "Algoritmo local",
    ),
    dynamic_translation("Smooth", "Lisser", "Glätten", "Suavizar"),
    dynamic_translation("Smooth N", "Lissage N", "Glättung N", "Suavizado N"),
    dynamic_translation(
        "Psychoacoustic",
        "Psychoacoustique",
        "Psychoakustisch",
        "Psicoacústico",
    ),
    dynamic_translation("BO Initial", "BO initial", "BO initial", "BO inicial"),
    dynamic_translation("BO Batch", "Lot BO", "BO-Batch", "Lote BO"),
    dynamic_translation(
        "BO Std Stop",
        "Arrêt écart-type BO",
        "BO-Std.-Stopp",
        "Parada desv. BO",
    ),
    dynamic_translation(
        "BO Acquisition",
        "Acquisition BO",
        "BO-Akquisition",
        "Adquisición BO",
    ),
    dynamic_translation("BO qEHVI", "BO qEHVI", "BO qEHVI", "BO qEHVI"),
    dynamic_translation(
        "Asymmetric Loss",
        "Perte asymétrique",
        "Asymmetrischer Verlust",
        "Pérdida asimétrica",
    ),
    dynamic_translation("Mode", "Mode", "Modus", "Modo"),
    dynamic_translation(
        "Multi-Speaker",
        "Multi-enceintes",
        "Mehrere Lautsprecher",
        "Multialtavoz",
    ),
    dynamic_translation(
        "Target Response",
        "Réponse cible",
        "Zielantwort",
        "Respuesta objetivo",
    ),
    dynamic_translation(
        "Slope (dB/oct)",
        "Pente (dB/oct)",
        "Steigung (dB/Okt.)",
        "Pendiente (dB/oct)",
    ),
    dynamic_translation(
        "Excursion Prot.",
        "Protection d’excursion",
        "Auslenkungsschutz",
        "Protección de excursión",
    ),
    dynamic_translation(
        "Manual F3 (Hz)",
        "F3 manuel (Hz)",
        "Manuelle F3 (Hz)",
        "F3 manual (Hz)",
    ),
    dynamic_translation(
        "Schroeder Split",
        "Séparation de Schroeder",
        "Schroeder-Trennung",
        "División de Schroeder",
    ),
    dynamic_translation(
        "Schroeder Freq",
        "Fréq. de Schroeder",
        "Schroeder-Frequenz",
        "Frec. de Schroeder",
    ),
    dynamic_translation(
        "Phase Alignment",
        "Alignement de phase",
        "Phasenausrichtung",
        "Alineación de fase",
    ),
    dynamic_translation(
        "Spacing Weight",
        "Poids d’espacement",
        "Abstandsgewicht",
        "Peso de espaciado",
    ),
    dynamic_translation(
        "Min Spacing (oct)",
        "Espacement min. (oct)",
        "Min. Abstand (Okt.)",
        "Espaciado mín. (oct)",
    ),
    dynamic_translation("Tolerance", "Tolérance", "Toleranz", "Tolerancia"),
    dynamic_translation(
        "Abs Tolerance",
        "Tolérance abs.",
        "Abs. Toleranz",
        "Tolerancia abs.",
    ),
    dynamic_translation(
        "Sample Rate",
        "Fréquence d’échantillonnage",
        "Abtastrate",
        "Frecuencia de muestreo",
    ),
    dynamic_translation("Type", "Type", "Typ", "Tipo"),
    dynamic_translation("Freq", "Fréq.", "Freq.", "Frec."),
    dynamic_translation("Q", "Q", "Q", "Q"),
    dynamic_translation("dB", "dB", "dB", "dB"),
    dynamic_translation(
        " 5-7 for quick results, 10+ for surgical precision",
        " 5 à 7 pour un résultat rapide, 10+ pour une grande précision",
        " 5–7 für schnelle Ergebnisse, 10+ für höchste Präzision",
        " 5-7 para resultados rápidos, 10+ para máxima precisión",
    ),
    dynamic_translation(
        " Narrow the range to the problem region for faster results",
        " Réduisez la plage à la zone problématique pour accélérer",
        " Bereich für schnellere Ergebnisse auf die Problemzone begrenzen",
        " Limite el rango a la zona problemática para acelerar",
    ),
    dynamic_translation(
        " Left/Right to cycle filter types",
        " Gauche/Droite pour parcourir les types de filtre",
        " Links/Rechts wechselt den Filtertyp",
        " Izquierda/Derecha cambia el tipo de filtro",
    ),
    dynamic_translation(
        " Left/Right to cycle loss functions",
        " Gauche/Droite pour parcourir les fonctions de perte",
        " Links/Rechts wechselt die Verlustfunktion",
        " Izquierda/Derecha cambia la función de pérdida",
    ),
    dynamic_translation(
        " Left/Right to change preset",
        " Gauche/Droite pour changer de préréglage",
        " Links/Rechts wechselt das Preset",
        " Izquierda/Derecha cambia el preajuste",
    ),
    dynamic_translation(
        " Up/Down=navigate  Left/Right=adjust  Enter=edit  Tab=cycle mode",
        " Haut/Bas=parcourir  Gauche/Droite=régler  Entrée=modifier  Tab=changer de mode",
        " Hoch/Runter=Navigieren  Links/Rechts=Anpassen  Eingabe=Bearbeiten  Tab=Modus",
        " Arriba/Abajo=navegar  Izq./Der.=ajustar  Intro=editar  Tab=modo",
    ),
    dynamic_translation(
        " Up/Down=navigate  Left/Right=adjust  Enter=edit value  Tab=next field",
        " Haut/Bas=parcourir  Gauche/Droite=régler  Entrée=modifier  Tab=champ suivant",
        " Hoch/Runter=Navigieren  Links/Rechts=Anpassen  Eingabe=Bearbeiten  Tab=Nächstes Feld",
        " Arriba/Abajo=navegar  Izq./Der.=ajustar  Intro=editar  Tab=campo siguiente",
    ),
    dynamic_translation(
        "Ready to optimize. Press Enter to start.",
        "Prêt à optimiser. Appuyez sur Entrée pour démarrer.",
        "Optimierungsbereit. Eingabe zum Starten drücken.",
        "Listo para optimizar. Pulse Intro para iniciar.",
    ),
    dynamic_translation(
        "Starting optimization...",
        "Démarrage de l’optimisation...",
        "Optimierung wird gestartet...",
        "Iniciando optimización...",
    ),
    dynamic_translation(
        "Processing {}...",
        "Traitement de {}...",
        "{} wird verarbeitet...",
        "Procesando {}...",
    ),
    dynamic_translation(
        "Optimizing... iter {}/{} | loss: {}",
        "Optimisation... itér. {}/{} | perte : {}",
        "Optimierung... Iter. {}/{} | Verlust: {}",
        "Optimizando... iter. {}/{} | pérdida: {}",
    ),
    dynamic_translation(
        "Optimizing... iter {}/{} | loss: {}{}",
        "Optimisation... itér. {}/{} | perte : {}{}",
        "Optimierung... Iter. {}/{} | Verlust: {}{}",
        "Optimizando... iter. {}/{} | pérdida: {}{}",
    ),
    dynamic_translation(
        "Completed! Final loss: {} | {} filters",
        "Terminé ! Perte finale : {} | {} filtres",
        "Fertig! Endverlust: {} | {} Filter",
        "¡Completado! Pérdida final: {} | {} filtros",
    ),
    dynamic_translation(
        "Completed! {} channel results",
        "Terminé ! Résultats pour {} canaux",
        "Fertig! Ergebnisse für {} Kanäle",
        "¡Completado! Resultados de {} canales",
    ),
    dynamic_translation("Cancelled", "Annulé", "Abgebrochen", "Cancelado"),
    dynamic_translation(
        "unknown error",
        "erreur inconnue",
        "unbekannter Fehler",
        "error desconocido",
    ),
    dynamic_translation(
        " Enter=start  BackTab=back to configure",
        " Entrée=démarrer  RetourTab=revenir à la configuration",
        " Eingabe=Start  Umschalt+Tab=Zurück zur Konfiguration",
        " Intro=iniciar  Mayús+Tab=volver a configuración",
    ),
    dynamic_translation(
        " Optimization running...",
        " Optimisation en cours...",
        " Optimierung läuft...",
        " Optimización en curso...",
    ),
    dynamic_translation(
        " Enter or Tab=view results",
        " Entrée ou Tab=voir les résultats",
        " Eingabe oder Tab=Ergebnisse",
        " Intro o Tab=ver resultados",
    ),
    dynamic_translation(
        " Enter=retry  BackTab=back to configure",
        " Entrée=réessayer  RetourTab=revenir à la configuration",
        " Eingabe=Erneut versuchen  Umschalt+Tab=Zurück",
        " Intro=reintentar  Mayús+Tab=volver",
    ),
    dynamic_translation(" {} filters", " {} filtres", " {} Filter", " {} filtros"),
    dynamic_translation(
        "Loss: {} → {} (Δ {})",
        "Perte : {} → {} (Δ {})",
        "Verlust: {} → {} (Δ {})",
        "Pérdida: {} → {} (Δ {})",
    ),
    dynamic_translation("(none)", "(aucun)", "(keine)", "(ninguno)"),
    dynamic_translation(
        "  {} PEQ filters ready to apply",
        "  {} filtres PEQ prêts à appliquer",
        "  {} PEQ-Filter können angewendet werden",
        "  {} filtros PEQ listos para aplicar",
    ),
    dynamic_translation(
        "  Press Enter to apply filters to the EQ plugin in the rack.",
        "  Appuyez sur Entrée pour appliquer les filtres au module EQ du rack.",
        "  Eingabe wendet die Filter auf das EQ-Plugin im Rack an.",
        "  Pulse Intro para aplicar los filtros al complemento EQ del rack.",
    ),
    dynamic_translation(
        "  If no EQ plugin exists it will be added automatically.",
        "  Si aucun module EQ n’existe, il sera ajouté automatiquement.",
        "  Falls kein EQ-Plugin existiert, wird es automatisch hinzugefügt.",
        "  Si no existe un complemento EQ, se añadirá automáticamente.",
    ),
    dynamic_translation(
        "  No optimization results yet. Run optimization first.",
        "  Aucun résultat d’optimisation. Lancez d’abord l’optimisation.",
        "  Noch keine Optimierungsergebnisse. Zuerst optimieren.",
        "  Aún no hay resultados. Ejecute primero la optimización.",
    ),
    dynamic_translation(
        " Enter=apply to rack  ←/BackTab=Results",
        " Entrée=appliquer au rack  ←/RetourTab=Résultats",
        " Eingabe=Auf Rack anwenden  ←/Umschalt+Tab=Ergebnisse",
        " Intro=aplicar al rack  ←/Mayús+Tab=Resultados",
    ),
    dynamic_translation(
        " Enter=apply to rack  →=Select  ←/BackTab=Results",
        " Entrée=appliquer au rack  →=Sélectionner  ←/RetourTab=Résultats",
        " Eingabe=Auf Rack anwenden  →=Auswählen  ←/Umschalt+Tab=Ergebnisse",
        " Intro=aplicar al rack  →=Seleccionar  ←/Mayús+Tab=Resultados",
    ),
    dynamic_translation(
        "  Existing EQ in slot {} has {} filter(s).",
        "  L’EQ de l’emplacement {} contient {} filtre(s).",
        "  Der EQ in Slot {} hat {} Filter.",
        "  El EQ de la posición {} tiene {} filtro(s).",
    ),
    dynamic_translation(
        "  Save current preset before overwriting?",
        "  Enregistrer le préréglage actuel avant de l’écraser ?",
        "  Aktuelles Preset vor dem Überschreiben speichern?",
        "  ¿Guardar el preajuste actual antes de sobrescribir?",
    ),
    dynamic_translation(
        " = save preset then apply   ",
        " = enregistrer puis appliquer   ",
        " = Preset speichern, dann anwenden   ",
        " = guardar preajuste y aplicar   ",
    ),
    dynamic_translation(
        " = apply without saving   ",
        " = appliquer sans enregistrer   ",
        " = ohne Speichern anwenden   ",
        " = aplicar sin guardar   ",
    ),
    dynamic_translation(
        "Measurements JSON (editing)",
        "JSON de mesures (édition)",
        "Messdaten-JSON (Bearbeitung)",
        "JSON de mediciones (editando)",
    ),
    dynamic_translation(
        "Measurements JSON",
        "JSON de mesures",
        "Messdaten-JSON",
        "JSON de mediciones",
    ),
    dynamic_translation(
        "<type path to recordings.json>",
        "<saisir le chemin vers recordings.json>",
        "<Pfad zu recordings.json eingeben>",
        "<escriba la ruta a recordings.json>",
    ),
    dynamic_translation(
        " {} channels loaded",
        " {} canaux chargés",
        " {} Kanäle geladen",
        " {} canales cargados",
    ),
    dynamic_translation("{} pts", "{} points", "{} Punkte", "{} puntos"),
    dynamic_translation("Group", "Groupe", "Gruppe", "Grupo"),
    dynamic_translation("Single", "Unique", "Einzeln", "Individual"),
    dynamic_translation(
        " Enter=confirm  F2=browse  Tab=autocomplete  Esc=cancel",
        " Entrée=confirmer  F2=parcourir  Tab=compléter  Échap=annuler",
        " Eingabe=Bestätigen  F2=Durchsuchen  Tab=Vervollständigen  Esc=Abbrechen",
        " Intro=confirmar  F2=examinar  Tab=completar  Esc=cancelar",
    ),
    dynamic_translation(
        " Enter=browse for JSON  Tab=next step",
        " Entrée=choisir le JSON  Tab=étape suivante",
        " Eingabe=JSON auswählen  Tab=Nächster Schritt",
        " Intro=buscar JSON  Tab=paso siguiente",
    ),
    dynamic_translation(
        "{}Simple Wizard — {}\n  Guided preset for {}.\n  3=2.0  4=2.1  5=5.1",
        "{}Assistant simple — {}\n  Préréglage guidé pour {}.\n  3=2.0  4=2.1  5=5.1",
        "{}Einfacher Assistent — {}\n  Geführtes Preset für {}.\n  3=2.0  4=2.1  5=5.1",
        "{}Asistente simple — {}\n  Preajuste guiado para {}.\n  3=2.0  4=2.1  5=5.1",
    ),
    dynamic_translation(
        "{}Full Wizard\n  All parameters in Acoustic + Optimizer blocks.\n  Full control over every setting.",
        "{}Assistant complet\n  Tous les paramètres dans les blocs Acoustique + Optimiseur.\n  Contrôle complet de chaque réglage.",
        "{}Vollständiger Assistent\n  Alle Parameter in Akustik- und Optimierer-Blöcken.\n  Volle Kontrolle über alle Einstellungen.",
        "{}Asistente completo\n  Todos los parámetros en Acústica + Optimizador.\n  Control total de cada ajuste.",
    ),
    dynamic_translation("No data", "Aucune donnée", "Keine Daten", "Sin datos"),
    dynamic_translation(
        "  Slope: {} dB/oct  |  Rec: [{}, {}] dB/oct",
        "  Pente : {} dB/oct  |  Recommandé : [{}, {}] dB/oct",
        "  Steigung: {} dB/Okt.  |  Empf.: [{}, {}] dB/Okt.",
        "  Pendiente: {} dB/oct  |  Rec.: [{}, {}] dB/oct",
    ),
    dynamic_translation(
        "Waiting for optimization...",
        "En attente de l’optimisation...",
        "Warten auf Optimierung...",
        "Esperando optimización...",
    ),
    dynamic_translation(
        "Waiting for loss data...",
        "En attente des données de perte...",
        "Warten auf Verlustdaten...",
        "Esperando datos de pérdida...",
    ),
    dynamic_translation(
        "No loss data recorded",
        "Aucune donnée de perte enregistrée",
        "Keine Verlustdaten aufgezeichnet",
        "No hay datos de pérdida",
    ),
    dynamic_translation(
        "Logs ({} lines)",
        "Journaux ({} lignes)",
        "Protokoll ({} Zeilen)",
        "Registros ({} líneas)",
    ),
    dynamic_translation(
        " Enter=start  BackTab=configure",
        " Entrée=démarrer  RetourTab=configurer",
        " Eingabe=Start  Umschalt+Tab=Konfiguration",
        " Intro=iniciar  Mayús+Tab=configurar",
    ),
    dynamic_translation(
        " Enter=re-run  Tab=view results  BackTab=configure",
        " Entrée=relancer  Tab=résultats  RetourTab=configurer",
        " Eingabe=Neu starten  Tab=Ergebnisse  Umschalt+Tab=Konfiguration",
        " Intro=repetir  Tab=resultados  Mayús+Tab=configurar",
    ),
    dynamic_translation(
        " Enter=retry  BackTab=configure",
        " Entrée=réessayer  RetourTab=configurer",
        " Eingabe=Erneut versuchen  Umschalt+Tab=Konfiguration",
        " Intro=reintentar  Mayús+Tab=configurar",
    ),
    dynamic_translation(
        " j/k=scroll logs  Enter=start/re-run  BackTab=configure",
        " j/k=faire défiler  Entrée=démarrer/relancer  RetourTab=configurer",
        " j/k=Protokoll scrollen  Eingabe=Start/Neustart  Umschalt+Tab=Konfiguration",
        " j/k=desplazar registros  Intro=iniciar/repetir  Mayús+Tab=configurar",
    ),
    dynamic_translation("Filters: {}", "Filtres : {}", "Filter: {}", "Filtros: {}"),
    dynamic_translation(
        " a=Apply to Rack (linear EQ)",
        " a=Appliquer au rack (EQ linéaire)",
        " a=Auf Rack anwenden (linearer EQ)",
        " a=Aplicar al rack (EQ lineal)",
    ),
    dynamic_translation(
        " a=Apply as Graph (multi-driver / routed)",
        " a=Appliquer comme graphe (multi-voie / routé)",
        " a=Als Graph anwenden (Mehrwege/geroutet)",
        " a=Aplicar como grafo (multivía/enrutado)",
    ),
    dynamic_translation(
        " a=Apply (run optimizer first)",
        " a=Appliquer (lancer d’abord l’optimiseur)",
        " a=Anwenden (zuerst optimieren)",
        " a=Aplicar (ejecute antes el optimizador)",
    ),
    dynamic_translation(
        "Export Path (editing)",
        "Chemin d’export (édition)",
        "Exportpfad (Bearbeitung)",
        "Ruta de exportación (editando)",
    ),
    dynamic_translation(
        "Export Path",
        "Chemin d’export",
        "Exportpfad",
        "Ruta de exportación",
    ),
    dynamic_translation(
        "<type path for JSON export>",
        "<saisir le chemin d’export JSON>",
        "<Pfad für JSON-Export eingeben>",
        "<escriba la ruta de exportación JSON>",
    ),
    dynamic_translation(
        " Enter=edit/export  a=Apply to chain  Tab=back to load  BackTab=review",
        " Entrée=modifier/exporter  a=appliquer à la chaîne  Tab=charger  RetourTab=examiner",
        " Eingabe=Bearbeiten/Export  a=Auf Kette anwenden  Tab=Laden  Umschalt+Tab=Prüfen",
        " Intro=editar/exportar  a=aplicar a cadena  Tab=cargar  Mayús+Tab=revisar",
    ),
    dynamic_translation(
        " ⚠ Delays < 0.3 ms — consider using 0. Delays auto-feed into optimizer.",
        " ⚠ Délais < 0,3 ms — envisagez 0. Les délais alimentent automatiquement l’optimiseur.",
        " ⚠ Verzögerungen < 0,3 ms — ggf. 0 verwenden. Werte fließen automatisch in die Optimierung.",
        " ⚠ Retardos < 0,3 ms — considere usar 0. Se pasan automáticamente al optimizador.",
    ),
    dynamic_translation(
        " Delays auto-feed into optimizer. j/k=row  e=edit  Tab=next step",
        " Les délais alimentent l’optimiseur. j/k=ligne  e=modifier  Tab=étape suivante",
        " Verzögerungen fließen in die Optimierung. j/k=Zeile  e=Bearbeiten  Tab=Nächster Schritt",
        " Los retardos pasan al optimizador. j/k=fila  e=editar  Tab=paso siguiente",
    ),
    dynamic_translation(
        " No delay data. Run the Probe step in the Recording wizard,\n or load a file with probe results.",
        " Aucune donnée de délai. Lancez l’étape Sonde de l’assistant Enregistrement,\n ou chargez un fichier contenant des résultats.",
        " Keine Verzögerungsdaten. Sondenschritt im Aufnahmeassistenten ausführen\n oder eine Datei mit Ergebnissen laden.",
        " No hay datos de retardo. Ejecute Sonda en el asistente de Grabación\n o cargue un archivo con resultados.",
    ),
    dynamic_translation(
        "Search Speaker (loading...)",
        "Recherche d’enceinte (chargement...)",
        "Lautsprechersuche (Laden...)",
        "Buscar altavoz (cargando...)",
    ),
    dynamic_translation(
        "Search Speaker (type to filter, Enter to select)",
        "Rechercher une enceinte (saisir pour filtrer, Entrée pour choisir)",
        "Lautsprecher suchen (Filtern, Eingabe zum Auswählen)",
        "Buscar altavoz (escriba para filtrar, Intro para seleccionar)",
    ),
    dynamic_translation(
        "Speakers (press 'r' to load from spinorama.org)",
        "Enceintes (appuyez sur 'r' pour charger depuis spinorama.org)",
        "Lautsprecher ('r' lädt von spinorama.org)",
        "Altavoces (pulse 'r' para cargar desde spinorama.org)",
    ),
    dynamic_translation(
        "Speakers ({}/{})",
        "Enceintes ({}/{})",
        "Lautsprecher ({}/{})",
        "Altavoces ({}/{})",
    ),
    dynamic_translation(
        " Selected: {}  |  ←/→=step  Enter=confirm",
        " Sélectionné : {}  |  ←/→=étape  Entrée=confirmer",
        " Ausgewählt: {}  |  ←/→=Schritt  Eingabe=Bestätigen",
        " Seleccionado: {}  |  ←/→=paso  Intro=confirmar",
    ),
    dynamic_translation(
        " ←/→=step  ↑/↓=navigate  Enter=select  r=load speakers",
        " ←/→=étape  ↑/↓=parcourir  Entrée=sélectionner  r=charger les enceintes",
        " ←/→=Schritt  ↑/↓=Navigieren  Eingabe=Auswählen  r=Laden",
        " ←/→=paso  ↑/↓=navegar  Intro=seleccionar  r=cargar altavoces",
    ),
    dynamic_translation(
        "(no speaker selected)",
        "(aucune enceinte sélectionnée)",
        "(kein Lautsprecher ausgewählt)",
        "(ningún altavoz seleccionado)",
    ),
    dynamic_translation(
        " ↑/↓=select field  Left/Right=adjust  Enter=edit value  Tab=next field",
        " ↑/↓=champ  Gauche/Droite=régler  Entrée=modifier  Tab=champ suivant",
        " ↑/↓=Feld  Links/Rechts=Anpassen  Eingabe=Bearbeiten  Tab=Nächstes Feld",
        " ↑/↓=campo  Izq./Der.=ajustar  Intro=editar  Tab=campo siguiente",
    ),
    dynamic_translation(
        "Press Enter to start optimization",
        "Appuyez sur Entrée pour démarrer l’optimisation",
        "Eingabe startet die Optimierung",
        "Pulse Intro para iniciar la optimización",
    ),
    dynamic_translation(
        "Running... iter {}/{} | loss: {}",
        "En cours... itér. {}/{} | perte : {}",
        "Läuft... Iter. {}/{} | Verlust: {}",
        "En curso... iter. {}/{} | pérdida: {}",
    ),
    dynamic_translation(
        "Completed! Final loss: {}  |  {} filters found",
        "Terminé ! Perte finale : {}  |  {} filtres trouvés",
        "Fertig! Endverlust: {}  |  {} Filter gefunden",
        "¡Completado! Pérdida final: {}  |  {} filtros encontrados",
    ),
    dynamic_translation(
        " Enter=start  Tab=back to configure",
        " Entrée=démarrer  Tab=revenir à la configuration",
        " Eingabe=Start  Tab=Zurück zur Konfiguration",
        " Intro=iniciar  Tab=volver a configuración",
    ),
    dynamic_translation(
        " Enter=re-run  Tab=view results",
        " Entrée=relancer  Tab=voir les résultats",
        " Eingabe=Neu starten  Tab=Ergebnisse",
        " Intro=repetir  Tab=ver resultados",
    ),
    dynamic_translation(
        " Enter=retry  Tab=back to configure",
        " Entrée=réessayer  Tab=revenir à la configuration",
        " Eingabe=Erneut versuchen  Tab=Zurück",
        " Intro=reintentar  Tab=volver",
    ),
    dynamic_translation(
        "  |  Score: {} → {} (Δ {})",
        "  |  Score : {} → {} (Δ {})",
        "  |  Wert: {} → {} (Δ {})",
        "  |  Puntuación: {} → {} (Δ {})",
    ),
    dynamic_translation(
        " {} filters  |  Loss: {} → {} (Δ {}){}",
        " {} filtres  |  Perte : {} → {} (Δ {}){}",
        " {} Filter  |  Verlust: {} → {} (Δ {}){}",
        " {} filtros  |  Pérdida: {} → {} (Δ {}){}",
    ),
    dynamic_translation(
        "[READ-ONLY] ",
        "[LECTURE SEULE] ",
        "[SCHREIBGESCHÜTZT] ",
        "[SOLO LECTURA] ",
    ),
    dynamic_translation("Now: {}", "Lecture : {}", "Aktuell: {}", "Ahora: {}"),
    dynamic_translation(
        "Plugins: {} [updating...] ",
        "Modules : {} [mise à jour...] ",
        "Plugins: {} [Aktualisierung...] ",
        "Complementos: {} [actualizando...] ",
    ),
    dynamic_translation(
        "Plugins: {} ",
        "Modules : {} ",
        "Plugins: {} ",
        "Complementos: {} ",
    ),
    dynamic_translation("CLIP ", "ÉCRÊTAGE ", "CLIPPING ", "SATURACIÓN "),
    dynamic_translation(
        "Waveform {}/{}/{}",
        "Forme d’onde {}/{}/{}",
        "Wellenform {}/{}/{}",
        "Forma de onda {}/{}/{}",
    ),
    dynamic_translation(
        "AlbumGain {}/{}",
        "Gain album {}/{}",
        "Album-Gain {}/{}",
        "Ganancia de álbum {}/{}",
    ),
    dynamic_translation(
        "ReplayGain {}/{}/{}",
        "ReplayGain {}/{}/{}",
        "ReplayGain {}/{}/{}",
        "ReplayGain {}/{}/{}",
    ),
    dynamic_translation(
        "Bliss {}/{}/{}",
        "Bliss {}/{}/{}",
        "Bliss {}/{}/{}",
        "Bliss {}/{}/{}",
    ),
    dynamic_translation(
        "Library {}",
        "Bibliothèque {}",
        "Bibliothek {}",
        "Biblioteca {}",
    ),
    dynamic_translation(
        "[paused] {} ",
        "[pause] {} ",
        "[pausiert] {} ",
        "[pausado] {} ",
    ),
    dynamic_translation("=Help", "=Aide", "=Hilfe", "=Ayuda"),
    dynamic_translation(
        "Channel {} recording complete",
        "Enregistrement du canal {} terminé",
        "Aufnahme von Kanal {} abgeschlossen",
        "Grabación del canal {} completada",
    ),
    dynamic_translation(
        "Recorded {} CTC ear channels",
        "{} canaux auriculaires CTC enregistrés",
        "{} CTC-Ohrkanäle aufgenommen",
        "{} canales CTC de oído grabados",
    ),
    dynamic_translation(
        "Recording failed: {}",
        "Échec de l’enregistrement : {}",
        "Aufnahme fehlgeschlagen: {}",
        "Error de grabación: {}",
    ),
    dynamic_translation(
        "Save thread terminated without result",
        "Le thread d’enregistrement s’est terminé sans résultat",
        "Speicher-Thread ohne Ergebnis beendet",
        "El hilo de guardado terminó sin resultado",
    ),
    dynamic_translation(
        "Save name must not contain path separators",
        "Le nom ne doit pas contenir de séparateur de chemin",
        "Der Name darf keine Pfadtrenner enthalten",
        "El nombre no debe contener separadores de ruta",
    ),
    dynamic_translation(
        "No completed recordings to save",
        "Aucun enregistrement terminé à sauvegarder",
        "Keine abgeschlossenen Aufnahmen zum Speichern",
        "No hay grabaciones completadas para guardar",
    ),
    dynamic_translation(
        "Cannot create directory: {}",
        "Impossible de créer le dossier : {}",
        "Verzeichnis kann nicht erstellt werden: {}",
        "No se puede crear el directorio: {}",
    ),
    dynamic_translation(
        "Raw-sweep CTC mixes sweep ranges; falling back to measured CTC",
        "Le CTC par balayage brut mélange les plages ; utilisation du CTC mesuré",
        "Raw-Sweep-CTC mischt Sweep-Bereiche; gemessenes CTC wird verwendet",
        "El CTC de barrido bruto mezcla rangos; se usará el CTC medido",
    ),
    dynamic_translation(
        "Raw-sweep CTC incomplete; falling back to measured CTC",
        "CTC par balayage brut incomplet ; utilisation du CTC mesuré",
        "Raw-Sweep-CTC unvollständig; gemessenes CTC wird verwendet",
        "CTC de barrido bruto incompleto; se usará el CTC medido",
    ),
    dynamic_translation(
        "Could not export raw-sweep CTC transfer matrix: {}",
        "Impossible d’exporter la matrice CTC du balayage brut : {}",
        "Raw-Sweep-CTC-Matrix konnte nicht exportiert werden: {}",
        "No se pudo exportar la matriz CTC de barrido bruto: {}",
    ),
    dynamic_translation(
        "Could not export CTC transfer matrix: {}",
        "Impossible d’exporter la matrice de transfert CTC : {}",
        "CTC-Übertragungsmatrix konnte nicht exportiert werden: {}",
        "No se pudo exportar la matriz de transferencia CTC: {}",
    ),
    dynamic_translation(
        "Saved with measured CTC fallback; raw-sweep CTC was incomplete",
        "Enregistré avec le CTC mesuré ; le balayage brut était incomplet",
        "Mit gemessenem CTC gespeichert; Raw-Sweep-CTC war unvollständig",
        "Guardado con CTC medido; el barrido bruto estaba incompleto",
    ),
    dynamic_translation(
        "Set an output directory first",
        "Définissez d’abord un dossier de sortie",
        "Zuerst ein Ausgabeverzeichnis festlegen",
        "Defina primero un directorio de salida",
    ),
    dynamic_translation(
        "Raw-sweep CTC requires two ear input channels for the selected speaker/position",
        "Le CTC par balayage brut exige deux canaux d’entrée auriculaires pour l’enceinte/position choisie",
        "Raw-Sweep-CTC benötigt zwei Ohr-Eingangskanäle für Lautsprecher/Position",
        "El CTC de barrido bruto requiere dos canales de oído para el altavoz/posición",
    ),
    dynamic_translation(
        "Recording CTC ear pair for {}...",
        "Enregistrement de la paire auriculaire CTC pour {}...",
        "CTC-Ohrpaar für {} wird aufgenommen...",
        "Grabando par de oído CTC para {}...",
    ),
    dynamic_translation(
        "Recording channel {}...",
        "Enregistrement du canal {}...",
        "Kanal {} wird aufgenommen...",
        "Grabando canal {}...",
    ),
    dynamic_translation(
        "Error generating signal: {}",
        "Erreur de génération du signal : {}",
        "Fehler bei der Signalerzeugung: {}",
        "Error al generar la señal: {}",
    ),
    dynamic_translation(
        "Error writing temp WAV: {}",
        "Erreur d’écriture du WAV temporaire : {}",
        "Fehler beim Schreiben der temporären WAV-Datei: {}",
        "Error al escribir el WAV temporal: {}",
    ),
    dynamic_translation(
        "Could not write CTC reference sweep: {}",
        "Impossible d’écrire le balayage de référence CTC : {}",
        "CTC-Referenz-Sweep konnte nicht geschrieben werden: {}",
        "No se pudo escribir el barrido de referencia CTC: {}",
    ),
    dynamic_translation(
        "No file path specified",
        "Aucun chemin de fichier indiqué",
        "Kein Dateipfad angegeben",
        "No se indicó una ruta de archivo",
    ),
    dynamic_translation(
        "Read error: {}",
        "Erreur de lecture : {}",
        "Lesefehler: {}",
        "Error de lectura: {}",
    ),
    dynamic_translation(
        "No export path specified",
        "Aucun chemin d’export indiqué",
        "Kein Exportpfad angegeben",
        "No se indicó una ruta de exportación",
    ),
    dynamic_translation(
        "Format error: {}",
        "Erreur de format : {}",
        "Formatfehler: {}",
        "Error de formato: {}",
    ),
    dynamic_translation(
        "Write error: {}",
        "Erreur d’écriture : {}",
        "Schreibfehler: {}",
        "Error de escritura: {}",
    ),
    dynamic_translation(
        "No optimization results to apply. Run the optimizer first.",
        "Aucun résultat à appliquer. Lancez d’abord l’optimiseur.",
        "Keine Ergebnisse zum Anwenden. Zuerst optimieren.",
        "No hay resultados para aplicar. Ejecute primero el optimizador.",
    ),
    dynamic_translation(
        "No EQ filters found in optimization results",
        "Aucun filtre EQ trouvé dans les résultats",
        "Keine EQ-Filter in den Ergebnissen gefunden",
        "No se encontraron filtros EQ en los resultados",
    ),
    dynamic_translation(
        "Applied Room EQ to rack: {} channels, {} main filters, {} broadband",
        "EQ de salle appliqué au rack : {} canaux, {} filtres principaux, {} large bande",
        "Raum-EQ auf Rack angewendet: {} Kanäle, {} Hauptfilter, {} Breitband",
        "EQ de sala aplicado al rack: {} canales, {} filtros principales, {} de banda ancha",
    ),
    dynamic_translation(
        "Applied Room EQ as graph: {} nodes, {} edges",
        "EQ de salle appliqué comme graphe : {} nœuds, {} arêtes",
        "Raum-EQ als Graph angewendet: {} Knoten, {} Kanten",
        "EQ de sala aplicado como grafo: {} nodos, {} aristas",
    ),
    dynamic_translation(
        "No optimization results to apply",
        "Aucun résultat d’optimisation à appliquer",
        "Keine Optimierungsergebnisse zum Anwenden",
        "No hay resultados de optimización para aplicar",
    ),
    dynamic_translation(
        "Applied {} EQ filters for '{}' to plugin slot {}",
        "{} filtres EQ pour '{}' appliqués à l’emplacement {}",
        "{} EQ-Filter für '{}' auf Plugin-Slot {} angewendet",
        "{} filtros EQ para '{}' aplicados a la posición {}",
    ),
    dynamic_translation(
        "Saved backup: {}",
        "Sauvegarde enregistrée : {}",
        "Sicherung gespeichert: {}",
        "Copia guardada: {}",
    ),
    dynamic_translation(
        "Backup failed: {}",
        "Échec de la sauvegarde : {}",
        "Sicherung fehlgeschlagen: {}",
        "Error de copia: {}",
    ),
];

const FRENCH_ACTIONS: &[(&str, &str)] = &[
    (
        "Practice / Courses / Progress",
        "Pratique / Cours / Progression",
    ),
    (
        "Start or restart session",
        "Démarrer ou recommencer la session",
    ),
    (
        "Listen to original / filtered",
        "Écouter l’original / le signal filtré",
    ),
    ("Select answer", "Choisir la réponse"),
    ("Navigate courses", "Parcourir les cours"),
    ("Submit answer / next trial", "Valider / essai suivant"),
    (
        "Exercise / adaptive / boost-cut mode",
        "Exercice / adaptatif / mode accentuation-atténuation",
    ),
    (
        "Adjust bands, gain, Q, trials",
        "Régler bandes, gain, Q et essais",
    ),
    (
        "Add / previous / next training source",
        "Ajouter / source précédente / suivante",
    ),
    (
        "Set loop bounds / toggle loop",
        "Définir les bornes / activer la boucle",
    ),
    (
        "Return to library and clean audition path",
        "Retourner à la bibliothèque et nettoyer l’écoute",
    ),
    ("Practice/Courses/Progress", "Pratique/Cours/Progression"),
    ("Start session", "Démarrer la session"),
    ("Original/filtered", "Original/filtré"),
    ("Choose/submit", "Choisir/valider"),
    ("Exercise/adaptive/change", "Exercice/adaptatif/changement"),
    ("Loop controls", "Commandes de boucle"),
    ("", ""),
    (
        "(when editing a plugin)",
        "(pendant la modification d’un module)",
    ),
    (
        "(when on Directories sub-screen)",
        "(dans le sous-écran Dossiers)",
    ),
    (
        "(↑/↓ navigate, Enter select, Esc cancel)",
        "(↑/↓ parcourir, Entrée sélectionner, Échap annuler)",
    ),
    (
        "Add active playlist",
        "Ajouter à la liste de lecture active",
    ),
    (
        "Add album (or selected track) to active playlist",
        "Ajouter l’album ou la piste sélectionnée à la liste active",
    ),
    ("Add album to queue", "Ajouter l’album à la file"),
    ("Add directory", "Ajouter un dossier"),
    ("Add plugin", "Ajouter un module"),
    (
        "Add plugin (opens selection dialog)",
        "Ajouter un module (ouvre la sélection)",
    ),
    (
        "Adjust parameter value (large)",
        "Régler le paramètre par grands pas",
    ),
    (
        "Adjust parameter value (small)",
        "Régler le paramètre par petits pas",
    ),
    (
        "Analyze ReplayGain for all tracks",
        "Analyser ReplayGain pour toutes les pistes",
    ),
    ("Back", "Retour"),
    ("Browse", "Parcourir"),
    ("Clear", "Effacer"),
    ("Clear entire queue", "Vider toute la file"),
    (
        "Close playlist (back to list)",
        "Fermer la liste de lecture et revenir à la liste",
    ),
    (
        "Collapse/expand artists in tree view",
        "Réduire ou développer les artistes dans l’arborescence",
    ),
    ("Create new playlist", "Créer une liste de lecture"),
    ("Create/rename/delete", "Créer/renommer/supprimer"),
    (
        "Database maintenance (clean missing files)",
        "Entretenir la base (nettoyer les fichiers absents)",
    ),
    (
        "Delete selected playlist",
        "Supprimer la liste sélectionnée",
    ),
    ("Directories sub-screen", "Sous-écran Dossiers"),
    ("Edit", "Modifier"),
    ("Edit selected plugin", "Modifier le module sélectionné"),
    ("Enable/disable", "Activer/désactiver"),
    ("Exit edit mode", "Quitter le mode d’édition"),
    (
        "Expand/collapse album tracks",
        "Développer ou réduire les pistes de l’album",
    ),
    ("Export playlist to M3U", "Exporter la liste en M3U"),
    (
        "Federation Sources sub-screen",
        "Sous-écran Sources fédérées",
    ),
    ("Filter", "Filtrer"),
    (
        "Filter: All/Mono/Stereo/Surround/Mixed",
        "Filtrer : Tous/Mono/Stéréo/Surround/Mixte",
    ),
    (
        "Force rescan ALL files (preserves ReplayGain)",
        "Réanalyser TOUS les fichiers (conserve ReplayGain)",
    ),
    ("Go to queue screen", "Ouvrir l’écran de la file"),
    ("Headphone EQ sub-screen", "Sous-écran Égalisation casque"),
    ("Help", "Aide"),
    ("Import M3U playlist", "Importer une liste M3U"),
    ("Jump by page", "Avancer par page"),
    ("Jump to tab", "Ouvrir l’onglet"),
    ("Load", "Charger"),
    (
        "Load APO file (EQ plugins only)",
        "Charger un fichier APO (modules EQ uniquement)",
    ),
    (
        "Load SOFA file (Binaural only)",
        "Charger un fichier SOFA (binaural uniquement)",
    ),
    (
        "Load plugin chain from file",
        "Charger un fichier de chaîne",
    ),
    (
        "Metadata Services sub-screen",
        "Sous-écran Services de métadonnées",
    ),
    (
        "Move plugin down in chain",
        "Descendre le module dans la chaîne",
    ),
    ("Move plugin up in chain", "Monter le module dans la chaîne"),
    ("Move track up/down", "Monter ou descendre la piste"),
    (
        "Navigate albums/artists",
        "Parcourir les albums et artistes",
    ),
    ("Navigate directories", "Parcourir les dossiers"),
    (
        "Navigate output devices",
        "Parcourir les périphériques de sortie",
    ),
    ("Navigate parameters", "Parcourir les paramètres"),
    (
        "Navigate playlists/tracks",
        "Parcourir les listes et pistes",
    ),
    ("Navigate plugin chain", "Parcourir la chaîne de modules"),
    ("Navigate queue items", "Parcourir les éléments de la file"),
    ("Navigate tabs", "Parcourir les onglets"),
    ("Next track", "Piste suivante"),
    ("Open", "Ouvrir"),
    ("Open playlist", "Ouvrir la liste de lecture"),
    ("Open tab", "Ouvrir l’onglet"),
    ("Pause/resume", "Pause/Reprendre"),
    ("Play all", "Tout lire"),
    ("Play all tracks", "Lire toutes les pistes"),
    (
        "Play selected album from start",
        "Lire l’album sélectionné depuis le début",
    ),
    ("Play selection", "Lire la sélection"),
    ("Play/pause", "Lecture/Pause"),
    (
        "Play/resume from current position",
        "Lire ou reprendre à la position actuelle",
    ),
    ("Previous track", "Piste précédente"),
    ("Queue", "File d’attente"),
    ("Queue album", "Ajouter l’album à la file"),
    ("Recording sub-screen", "Sous-écran Enregistrement"),
    ("Remove from queue", "Retirer de la file"),
    ("Remove plugin", "Supprimer le module"),
    (
        "Remove selected directory",
        "Supprimer le dossier sélectionné",
    ),
    ("Remove track", "Supprimer la piste"),
    (
        "Remove track (in tracks view)",
        "Supprimer la piste (vue des pistes)",
    ),
    ("Rename selected playlist", "Renommer la liste sélectionnée"),
    ("Rescan", "Réanalyser"),
    (
        "Rescan audio and cast devices",
        "Réanalyser les périphériques audio et de diffusion",
    ),
    ("Room EQ sub-screen", "Sous-écran Correction de salle"),
    ("Save", "Sauvegarder"),
    (
        "Save plugin chain to file",
        "Sauvegarder la chaîne dans un fichier",
    ),
    (
        "Scan library (incremental)",
        "Analyser la bibliothèque (incrémental)",
    ),
    ("Search", "Rechercher"),
    ("Search albums", "Rechercher des albums"),
    ("Select", "Sélectionner"),
    ("Select output device", "Choisir le périphérique de sortie"),
    ("Servers sub-screen", "Sous-écran Serveurs"),
    ("Sort", "Trier"),
    (
        "Sort: cycle / Year / Genre / Artist / Album",
        "Tri : cycle / Année / Genre / Artiste / Album",
    ),
    (
        "Spinorama EQ sub-screen",
        "Sous-écran Égalisation Spinorama",
    ),
    (
        "Toggle plugin enabled/disabled",
        "Activer ou désactiver le module",
    ),
    (
        "Toggle tree view / flat view",
        "Basculer entre arborescence et liste",
    ),
    ("Tree/flat view", "Arborescence/liste"),
];

const GERMAN_ACTIONS: &[(&str, &str)] = &[
    (
        "Practice / Courses / Progress",
        "Übung / Kurse / Fortschritt",
    ),
    (
        "Start or restart session",
        "Sitzung starten oder neu starten",
    ),
    (
        "Listen to original / filtered",
        "Original / gefiltert anhören",
    ),
    ("Select answer", "Antwort auswählen"),
    ("Navigate courses", "Kurse auswählen"),
    (
        "Submit answer / next trial",
        "Antwort senden / nächste Runde",
    ),
    (
        "Exercise / adaptive / boost-cut mode",
        "Übung / adaptiv / Anhebung-Absenkung",
    ),
    (
        "Adjust bands, gain, Q, trials",
        "Bänder, Pegel, Q und Runden anpassen",
    ),
    (
        "Add / previous / next training source",
        "Quelle hinzufügen / vorherige / nächste",
    ),
    (
        "Set loop bounds / toggle loop",
        "Schleifengrenzen setzen / Schleife umschalten",
    ),
    (
        "Return to library and clean audition path",
        "Zur Mediathek und Hörpfad entfernen",
    ),
    ("Practice/Courses/Progress", "Übung/Kurse/Fortschritt"),
    ("Start session", "Sitzung starten"),
    ("Original/filtered", "Original/gefiltert"),
    ("Choose/submit", "Auswählen/senden"),
    ("Exercise/adaptive/change", "Übung/adaptiv/Änderung"),
    ("Loop controls", "Schleifensteuerung"),
    ("", ""),
    ("(when editing a plugin)", "(beim Bearbeiten eines Plugins)"),
    (
        "(when on Directories sub-screen)",
        "(im Unterbereich Ordner)",
    ),
    (
        "(↑/↓ navigate, Enter select, Esc cancel)",
        "(↑/↓ navigieren, Eingabe wählen, Esc abbrechen)",
    ),
    (
        "Add active playlist",
        "Zur aktiven Wiedergabeliste hinzufügen",
    ),
    (
        "Add album (or selected track) to active playlist",
        "Album oder ausgewählten Titel zur aktiven Wiedergabeliste hinzufügen",
    ),
    ("Add album to queue", "Album zur Warteschlange hinzufügen"),
    ("Add directory", "Ordner hinzufügen"),
    ("Add plugin", "Plugin hinzufügen"),
    (
        "Add plugin (opens selection dialog)",
        "Plugin hinzufügen (öffnet die Auswahl)",
    ),
    (
        "Adjust parameter value (large)",
        "Parameter in großen Schritten ändern",
    ),
    (
        "Adjust parameter value (small)",
        "Parameter in kleinen Schritten ändern",
    ),
    (
        "Analyze ReplayGain for all tracks",
        "ReplayGain für alle Titel analysieren",
    ),
    ("Back", "Zurück"),
    ("Browse", "Durchsuchen"),
    ("Clear", "Leeren"),
    ("Clear entire queue", "Gesamte Warteschlange leeren"),
    (
        "Close playlist (back to list)",
        "Wiedergabeliste schließen und zur Liste zurückkehren",
    ),
    (
        "Collapse/expand artists in tree view",
        "Interpreten in der Baumansicht ein- oder ausklappen",
    ),
    ("Create new playlist", "Neue Wiedergabeliste erstellen"),
    ("Create/rename/delete", "Erstellen/umbenennen/löschen"),
    (
        "Database maintenance (clean missing files)",
        "Datenbank warten (fehlende Dateien bereinigen)",
    ),
    (
        "Delete selected playlist",
        "Ausgewählte Wiedergabeliste löschen",
    ),
    ("Directories sub-screen", "Unterbereich Ordner"),
    ("Edit", "Bearbeiten"),
    ("Edit selected plugin", "Ausgewähltes Plugin bearbeiten"),
    ("Enable/disable", "Aktivieren/deaktivieren"),
    ("Exit edit mode", "Bearbeitungsmodus verlassen"),
    (
        "Expand/collapse album tracks",
        "Albumtitel ein- oder ausklappen",
    ),
    (
        "Export playlist to M3U",
        "Wiedergabeliste als M3U exportieren",
    ),
    (
        "Federation Sources sub-screen",
        "Unterbereich Verbundquellen",
    ),
    ("Filter", "Filtern"),
    (
        "Filter: All/Mono/Stereo/Surround/Mixed",
        "Filtern: Alle/Mono/Stereo/Surround/Gemischt",
    ),
    (
        "Force rescan ALL files (preserves ReplayGain)",
        "ALLE Dateien neu scannen (ReplayGain bleibt erhalten)",
    ),
    ("Go to queue screen", "Warteschlange öffnen"),
    ("Headphone EQ sub-screen", "Unterbereich Kopfhörer-EQ"),
    ("Help", "Hilfe"),
    ("Import M3U playlist", "M3U-Wiedergabeliste importieren"),
    ("Jump by page", "Seitenweise springen"),
    ("Jump to tab", "Zum Tab springen"),
    ("Load", "Laden"),
    (
        "Load APO file (EQ plugins only)",
        "APO-Datei laden (nur EQ-Plugins)",
    ),
    (
        "Load SOFA file (Binaural only)",
        "SOFA-Datei laden (nur binaural)",
    ),
    ("Load plugin chain from file", "Plugin-Kettendatei laden"),
    (
        "Metadata Services sub-screen",
        "Unterbereich Metadatendienste",
    ),
    (
        "Move plugin down in chain",
        "Plugin in der Kette nach unten verschieben",
    ),
    (
        "Move plugin up in chain",
        "Plugin in der Kette nach oben verschieben",
    ),
    (
        "Move track up/down",
        "Titel nach oben oder unten verschieben",
    ),
    (
        "Navigate albums/artists",
        "Alben und Interpreten durchsuchen",
    ),
    ("Navigate directories", "Ordner durchsuchen"),
    ("Navigate output devices", "Ausgabegeräte durchsuchen"),
    ("Navigate parameters", "Parameter durchsuchen"),
    (
        "Navigate playlists/tracks",
        "Wiedergabelisten und Titel durchsuchen",
    ),
    ("Navigate plugin chain", "Plugin-Kette durchsuchen"),
    ("Navigate queue items", "Warteschlangenelemente durchsuchen"),
    ("Navigate tabs", "Tabs durchsuchen"),
    ("Next track", "Nächster Titel"),
    ("Open", "Öffnen"),
    ("Open playlist", "Wiedergabeliste öffnen"),
    ("Open tab", "Tab öffnen"),
    ("Pause/resume", "Pause/Fortsetzen"),
    ("Play all", "Alles abspielen"),
    ("Play all tracks", "Alle Titel abspielen"),
    (
        "Play selected album from start",
        "Ausgewähltes Album von Anfang an abspielen",
    ),
    ("Play selection", "Auswahl abspielen"),
    ("Play/pause", "Wiedergabe/Pause"),
    (
        "Play/resume from current position",
        "An aktueller Position abspielen oder fortsetzen",
    ),
    ("Previous track", "Vorheriger Titel"),
    ("Queue", "Warteschlange"),
    ("Queue album", "Album einreihen"),
    ("Recording sub-screen", "Unterbereich Aufnahme"),
    ("Remove from queue", "Aus Warteschlange entfernen"),
    ("Remove plugin", "Plugin entfernen"),
    ("Remove selected directory", "Ausgewählten Ordner entfernen"),
    ("Remove track", "Titel entfernen"),
    (
        "Remove track (in tracks view)",
        "Titel entfernen (Titelansicht)",
    ),
    (
        "Rename selected playlist",
        "Ausgewählte Wiedergabeliste umbenennen",
    ),
    ("Rescan", "Neu scannen"),
    (
        "Rescan audio and cast devices",
        "Audio- und Übertragungsgeräte neu scannen",
    ),
    ("Room EQ sub-screen", "Unterbereich Raum-EQ"),
    ("Save", "Speichern"),
    (
        "Save plugin chain to file",
        "Plugin-Kette in Datei speichern",
    ),
    (
        "Scan library (incremental)",
        "Mediathek scannen (inkrementell)",
    ),
    ("Search", "Suchen"),
    ("Search albums", "Alben suchen"),
    ("Select", "Auswählen"),
    ("Select output device", "Ausgabegerät auswählen"),
    ("Servers sub-screen", "Unterbereich Server"),
    ("Sort", "Sortieren"),
    (
        "Sort: cycle / Year / Genre / Artist / Album",
        "Sortierung: wechseln / Jahr / Genre / Interpret / Album",
    ),
    ("Spinorama EQ sub-screen", "Unterbereich Spinorama-EQ"),
    (
        "Toggle plugin enabled/disabled",
        "Plugin aktivieren oder deaktivieren",
    ),
    (
        "Toggle tree view / flat view",
        "Zwischen Baum- und Listenansicht wechseln",
    ),
    ("Tree/flat view", "Baum-/Listenansicht"),
];

const SPANISH_ACTIONS: &[(&str, &str)] = &[
    (
        "Practice / Courses / Progress",
        "Práctica / Cursos / Progreso",
    ),
    ("Start or restart session", "Iniciar o reiniciar la sesión"),
    (
        "Listen to original / filtered",
        "Escuchar original / filtrado",
    ),
    ("Select answer", "Seleccionar respuesta"),
    ("Navigate courses", "Navegar por los cursos"),
    (
        "Submit answer / next trial",
        "Enviar respuesta / siguiente prueba",
    ),
    (
        "Exercise / adaptive / boost-cut mode",
        "Ejercicio / adaptativo / realce-recorte",
    ),
    (
        "Adjust bands, gain, Q, trials",
        "Ajustar bandas, ganancia, Q y pruebas",
    ),
    (
        "Add / previous / next training source",
        "Añadir / fuente anterior / siguiente",
    ),
    (
        "Set loop bounds / toggle loop",
        "Definir límites / activar el bucle",
    ),
    (
        "Return to library and clean audition path",
        "Volver a la biblioteca y limpiar la escucha",
    ),
    ("Practice/Courses/Progress", "Práctica/Cursos/Progreso"),
    ("Start session", "Iniciar sesión"),
    ("Original/filtered", "Original/filtrado"),
    ("Choose/submit", "Elegir/enviar"),
    ("Exercise/adaptive/change", "Ejercicio/adaptativo/cambio"),
    ("Loop controls", "Controles de bucle"),
    ("", ""),
    ("(when editing a plugin)", "(al editar un complemento)"),
    (
        "(when on Directories sub-screen)",
        "(en la subpantalla Carpetas)",
    ),
    (
        "(↑/↓ navigate, Enter select, Esc cancel)",
        "(↑/↓ navegar, Intro seleccionar, Esc cancelar)",
    ),
    ("Add active playlist", "Añadir a la lista activa"),
    (
        "Add album (or selected track) to active playlist",
        "Añadir el álbum o pista seleccionada a la lista activa",
    ),
    ("Add album to queue", "Añadir el álbum a la cola"),
    ("Add directory", "Añadir carpeta"),
    ("Add plugin", "Añadir complemento"),
    (
        "Add plugin (opens selection dialog)",
        "Añadir complemento (abre el selector)",
    ),
    (
        "Adjust parameter value (large)",
        "Ajustar el parámetro en pasos grandes",
    ),
    (
        "Adjust parameter value (small)",
        "Ajustar el parámetro en pasos pequeños",
    ),
    (
        "Analyze ReplayGain for all tracks",
        "Analizar ReplayGain en todas las pistas",
    ),
    ("Back", "Volver"),
    ("Browse", "Recorrer"),
    ("Clear", "Vaciar"),
    ("Clear entire queue", "Vaciar toda la cola"),
    (
        "Close playlist (back to list)",
        "Cerrar la lista y volver al listado",
    ),
    (
        "Collapse/expand artists in tree view",
        "Contraer o expandir artistas en la vista de árbol",
    ),
    ("Create new playlist", "Crear lista"),
    ("Create/rename/delete", "Crear/renombrar/eliminar"),
    (
        "Database maintenance (clean missing files)",
        "Mantener la base de datos (limpiar archivos ausentes)",
    ),
    ("Delete selected playlist", "Eliminar la lista seleccionada"),
    ("Directories sub-screen", "Subpantalla Carpetas"),
    ("Edit", "Editar"),
    ("Edit selected plugin", "Editar el complemento seleccionado"),
    ("Enable/disable", "Activar/desactivar"),
    ("Exit edit mode", "Salir del modo de edición"),
    (
        "Expand/collapse album tracks",
        "Expandir o contraer las pistas del álbum",
    ),
    ("Export playlist to M3U", "Exportar la lista a M3U"),
    (
        "Federation Sources sub-screen",
        "Subpantalla Fuentes federadas",
    ),
    ("Filter", "Filtrar"),
    (
        "Filter: All/Mono/Stereo/Surround/Mixed",
        "Filtrar: Todo/Mono/Estéreo/Envolvente/Mixto",
    ),
    (
        "Force rescan ALL files (preserves ReplayGain)",
        "Reanalizar TODOS los archivos (conserva ReplayGain)",
    ),
    ("Go to queue screen", "Abrir la pantalla de cola"),
    ("Headphone EQ sub-screen", "Subpantalla EQ de auriculares"),
    ("Help", "Ayuda"),
    ("Import M3U playlist", "Importar lista M3U"),
    ("Jump by page", "Saltar por páginas"),
    ("Jump to tab", "Ir a la pestaña"),
    ("Load", "Cargar"),
    (
        "Load APO file (EQ plugins only)",
        "Cargar archivo APO (solo complementos EQ)",
    ),
    (
        "Load SOFA file (Binaural only)",
        "Cargar archivo SOFA (solo binaural)",
    ),
    ("Load plugin chain from file", "Cargar archivo de cadena"),
    (
        "Metadata Services sub-screen",
        "Subpantalla Servicios de metadatos",
    ),
    (
        "Move plugin down in chain",
        "Bajar el complemento en la cadena",
    ),
    (
        "Move plugin up in chain",
        "Subir el complemento en la cadena",
    ),
    ("Move track up/down", "Mover la pista arriba o abajo"),
    ("Navigate albums/artists", "Recorrer álbumes y artistas"),
    ("Navigate directories", "Recorrer carpetas"),
    ("Navigate output devices", "Recorrer dispositivos de salida"),
    ("Navigate parameters", "Recorrer parámetros"),
    ("Navigate playlists/tracks", "Recorrer listas y pistas"),
    (
        "Navigate plugin chain",
        "Recorrer la cadena de complementos",
    ),
    ("Navigate queue items", "Recorrer los elementos de la cola"),
    ("Navigate tabs", "Recorrer pestañas"),
    ("Next track", "Pista siguiente"),
    ("Open", "Abrir"),
    ("Open playlist", "Abrir lista"),
    ("Open tab", "Abrir pestaña"),
    ("Pause/resume", "Pausar/Reanudar"),
    ("Play all", "Reproducir todo"),
    ("Play all tracks", "Reproducir todas las pistas"),
    (
        "Play selected album from start",
        "Reproducir el álbum seleccionado desde el principio",
    ),
    ("Play selection", "Reproducir la selección"),
    ("Play/pause", "Reproducir/Pausar"),
    (
        "Play/resume from current position",
        "Reproducir o reanudar desde la posición actual",
    ),
    ("Previous track", "Pista anterior"),
    ("Queue", "Cola"),
    ("Queue album", "Añadir álbum a la cola"),
    ("Recording sub-screen", "Subpantalla Grabación"),
    ("Remove from queue", "Quitar de la cola"),
    ("Remove plugin", "Quitar complemento"),
    (
        "Remove selected directory",
        "Quitar la carpeta seleccionada",
    ),
    ("Remove track", "Quitar pista"),
    (
        "Remove track (in tracks view)",
        "Quitar pista (vista de pistas)",
    ),
    (
        "Rename selected playlist",
        "Renombrar la lista seleccionada",
    ),
    ("Rescan", "Reanalizar"),
    (
        "Rescan audio and cast devices",
        "Reanalizar dispositivos de audio y transmisión",
    ),
    ("Room EQ sub-screen", "Subpantalla EQ de sala"),
    ("Save", "Guardar"),
    (
        "Save plugin chain to file",
        "Guardar la cadena en un archivo",
    ),
    (
        "Scan library (incremental)",
        "Analizar biblioteca (incremental)",
    ),
    ("Search", "Buscar"),
    ("Search albums", "Buscar álbumes"),
    ("Select", "Seleccionar"),
    ("Select output device", "Seleccionar dispositivo de salida"),
    ("Servers sub-screen", "Subpantalla Servidores"),
    ("Sort", "Ordenar"),
    (
        "Sort: cycle / Year / Genre / Artist / Album",
        "Ordenar: alternar / Año / Género / Artista / Álbum",
    ),
    ("Spinorama EQ sub-screen", "Subpantalla EQ Spinorama"),
    (
        "Toggle plugin enabled/disabled",
        "Activar o desactivar el complemento",
    ),
    (
        "Toggle tree view / flat view",
        "Alternar entre vista de árbol y lista",
    ),
    ("Tree/flat view", "Vista de árbol/lista"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_locales_and_screen_names_are_complete() {
        assert_eq!(Language::ALL.len(), 4);
        for language in Language::ALL {
            let text = TuiTranslations::for_language(language);
            for screen in [
                Screen::Loading,
                Screen::Library,
                Screen::Queue,
                Screen::Playlists,
                Screen::Plugins,
                Screen::Devices,
                Screen::Configure,
            ] {
                assert!(!text.screen_name(screen).trim().is_empty());
                assert!(!text.help_title(screen).trim().is_empty());
            }
        }
    }

    #[test]
    fn locale_detection_and_cycle_are_deterministic() {
        assert_eq!(Language::from_locale("fr_CH.UTF-8"), Language::French);
        assert_eq!(Language::from_locale("de-DE"), Language::German);
        assert_eq!(Language::from_locale("es"), Language::Spanish);
        assert_eq!(Language::from_locale("C"), Language::English);
        assert_eq!(Language::Spanish.next(), Language::English);
    }

    #[test]
    fn static_and_action_catalogs_have_locale_key_parity() {
        let keys = |table: &[(&str, &str)]| {
            table
                .iter()
                .map(|(source, translation)| {
                    assert!(!source.is_empty() || translation.is_empty());
                    assert!(!translation.trim().is_empty() || source.is_empty());
                    source.to_string()
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(keys(FRENCH_UI), keys(GERMAN_UI));
        assert_eq!(keys(FRENCH_UI), keys(SPANISH_UI));
        assert_eq!(keys(FRENCH_ACTIONS), keys(GERMAN_ACTIONS));
        assert_eq!(keys(FRENCH_ACTIONS), keys(SPANISH_ACTIONS));
    }

    #[test]
    fn dynamic_catalog_is_unique_complete_and_preserves_values() {
        let mut sources = std::collections::HashSet::new();
        for entry in DYNAMIC_TRANSLATIONS {
            assert!(
                sources.insert(entry.english),
                "duplicate: {}",
                entry.english
            );
            assert!(!entry.english.is_empty());
            for target in [entry.french, entry.german, entry.spanish] {
                assert!(!target.trim().is_empty());
                assert_eq!(
                    entry.english.matches("{}").count(),
                    target.matches("{}").count(),
                    "placeholder drift for {}",
                    entry.english
                );
            }

            let sample = entry.english.replace("{}", "VALUE");
            for language in [Language::French, Language::German, Language::Spanish] {
                let localized = TuiTranslations::for_language(language).dynamic(sample.clone());
                assert!(!localized.is_empty());
                assert_eq!(
                    localized.matches("VALUE").count(),
                    entry.english.matches("{}").count(),
                    "dynamic value drift for {} in {:?}",
                    entry.english,
                    language
                );
            }
        }

        for language in [Language::French, Language::German, Language::Spanish] {
            let i18n = TuiTranslations::for_language(language);
            let failure = i18n.dynamic("Failed: opaque external detail".to_string());
            assert!(failure.contains("opaque external detail"));
            assert_eq!(
                i18n.dynamic_or_verbatim("opaque external detail"),
                "opaque external detail"
            );
        }
    }

    #[test]
    fn first_party_renderers_use_catalogued_literal_sinks() {
        const SOURCES: &[(&str, &str)] = &[
            ("album_list", include_str!("ui/draw_album_list.rs")),
            ("autocomplete", include_str!("ui/draw_autocomplete.rs")),
            ("configure", include_str!("ui/draw_configure/draw.rs")),
            ("directory", include_str!("ui/draw_directory.rs")),
            ("federation", include_str!("ui/draw_federation.rs")),
            ("file_explorer", include_str!("ui/draw_file_explorer.rs")),
            ("graphs", include_str!("ui/draw_graphs.rs")),
            ("headphone_eq", include_str!("ui/draw_headphoneeq/misc.rs")),
            ("loading", include_str!("ui/draw_loading.rs")),
            ("meters", include_str!("ui/draw_meters/draw.rs")),
            ("modals", include_str!("ui/draw_mod.rs")),
            ("playlists", include_str!("ui/draw_playlists.rs")),
            ("plugins", include_str!("ui/draw_plugins/draw.rs")),
            ("progress", include_str!("ui/draw_progress.rs")),
            ("queue", include_str!("ui/draw_queue.rs")),
            ("room_eq", include_str!("ui/draw_roomeq/draw.rs")),
            ("search_box", include_str!("ui/draw_search_box.rs")),
            ("servers", include_str!("ui/draw_servers.rs")),
            ("spinorama_eq", include_str!("ui/draw_spinorama/misc.rs")),
            ("status_bar", include_str!("ui/draw_status_bar.rs")),
            ("title", include_str!("ui/draw_title.rs")),
            ("transport", include_str!("ui/draw_transport.rs")),
            ("volume", include_str!("ui/draw_volume.rs")),
        ];
        const LITERAL_SINKS: &[&str] = &[
            ".title(",
            "Paragraph::new(",
            "Line::from(",
            "Span::raw(",
            "Span::styled(",
            "Cell::from(",
            "ListItem::new(",
        ];
        const ALLOWED_TECHNICAL_LITERALS: &[&str] = &[
            "-60",
            "0",
            "+6",
            "LUFS",
            "1",
            "[M]",
            "[S]",
            "[D]",
            "1k",
            "5k",
            "Hz",
            "dB",
            "SOTF Player",
            "SotF",
            "MusicBrainz",
            "  y",
            "n",
            "Esc",
            "Enter",
            "Tab",
            "ESC",
            "Space",
            "↑↓",
            "  TAB",
            "  L/Q/P/O/C",
            "  Shift+M",
            "  Alt+L",
            "  +/=",
            "  -/_",
            "  Ctrl+Left/Right",
            "  Left/Right",
            "  Up/Down",
            "  m/s",
            "  c",
            "  ESC",
            "  Shift+Left/Right",
            "  Shift+Up/Down",
            "  Shift+S",
            "  Shift+C",
            "  ?",
            "  Ctrl+Q/Cmd+Q",
            "Out\\\\In",
            "plugin_presets/",
            "  Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41",
            "  Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71",
        ];

        for (name, source) in SOURCES {
            for sink in LITERAL_SINKS {
                let mut search_offset = 0;
                while let Some(relative_start) = source[search_offset..].find(sink) {
                    let start = search_offset + relative_start;
                    let after_sink = &source[start + sink.len()..];
                    let trimmed = after_sink.trim_start();
                    if let Some(literal_body) = trimmed.strip_prefix('"') {
                        let mut escaped = false;
                        let end = literal_body
                            .char_indices()
                            .find_map(|(index, character)| {
                                if character == '"' && !escaped {
                                    return Some(index);
                                }
                                escaped = character == '\\' && !escaped;
                                if character != '\\' {
                                    escaped = false;
                                }
                                None
                            })
                            .unwrap_or(literal_body.len());
                        let literal = &literal_body[..end];
                        if literal.chars().any(char::is_alphabetic)
                            && !ALLOWED_TECHNICAL_LITERALS.contains(&literal)
                        {
                            let line_number = source[..start].lines().count();
                            panic!(
                                "unlocalized visible literal in {name}:{line_number}: {literal}"
                            );
                        }
                    }
                    search_offset = start + sink.len();
                }
            }

            let mut remaining = *source;
            while let Some(start) = remaining.find("i18n.ui(\"") {
                remaining = &remaining[start + "i18n.ui(\"".len()..];
                let end = remaining
                    .find("\")")
                    .unwrap_or_else(|| panic!("unterminated i18n.ui call in {name}"));
                let key = &remaining[..end];
                for language in [Language::French, Language::German, Language::Spanish] {
                    assert!(
                        !TuiTranslations::for_language(language)
                            .ui(key)
                            .trim()
                            .is_empty(),
                        "empty {language:?} copy for {key:?} in {name}"
                    );
                }
                remaining = &remaining[end + 2..];
            }
        }
    }

    #[test]
    fn central_status_producer_literals_are_catalogued() {
        const SOURCES: &[(&str, &str)] = &[
            ("scanner", include_str!("app/app_scanner.rs")),
            ("library", include_str!("app/app_library/misc.rs")),
            ("plugins", include_str!("app/app_plugins/misc.rs")),
            ("autocomplete", include_str!("app/app_autocomplete.rs")),
            ("main_misc", include_str!("main/misc.rs")),
            ("main_types", include_str!("main/types.rs")),
            ("directories", include_str!("events/conf_directories.rs")),
            ("federation", include_str!("events/conf_federation.rs")),
            ("file_explorer", include_str!("events/file_explorer.rs")),
            (
                "headphone_eq",
                include_str!("events/conf_headphoneeq/headphone.rs"),
            ),
            ("library_events", include_str!("events/library.rs")),
            ("metadata", include_str!("events/metadata.rs")),
            ("playlists", include_str!("events/playlists.rs")),
            ("plugin_events", include_str!("events/plugins/handle.rs")),
            ("queue", include_str!("events/queue.rs")),
            (
                "spinorama_eq",
                include_str!("events/conf_spinoramaeq/spinorama.rs"),
            ),
            ("devices", include_str!("events/devices.rs")),
        ];

        fn sample_format_string(template: &str) -> String {
            let mut sample = String::new();
            let mut remaining = template;
            while let Some(open) = remaining.find('{') {
                sample.push_str(&remaining[..open]);
                let Some(close) = remaining[open + 1..].find('}') else {
                    sample.push_str(&remaining[open..]);
                    return sample;
                };
                sample.push_str("VALUE");
                remaining = &remaining[open + close + 2..];
            }
            sample.push_str(remaining);
            sample
        }

        for (name, source) in SOURCES {
            for (start, _) in source.match_indices("ui.status_message") {
                let remainder = &source[start..];
                let mut in_string = false;
                let mut escaped = false;
                let end = remainder
                    .char_indices()
                    .find_map(|(index, character)| {
                        if in_string {
                            if character == '"' && !escaped {
                                in_string = false;
                            }
                            escaped = character == '\\' && !escaped;
                            if character != '\\' {
                                escaped = false;
                            }
                            None
                        } else if character == '"' {
                            in_string = true;
                            None
                        } else if character == ';' {
                            Some(index)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(remainder.len());
                let assignment = &remainder[..end];
                if !assignment.contains('=') {
                    continue;
                }
                let Some(quote) = assignment.find('"') else {
                    continue;
                };
                let literal_body = &assignment[quote + 1..];
                let mut escaped = false;
                let literal_end = literal_body
                    .char_indices()
                    .find_map(|(index, character)| {
                        if character == '"' && !escaped {
                            return Some(index);
                        }
                        escaped = character == '\\' && !escaped;
                        if character != '\\' {
                            escaped = false;
                        }
                        None
                    })
                    .unwrap_or(literal_body.len());
                let template = literal_body[..literal_end]
                    .replace("\\\"", "\"")
                    .replace("\\n", "\n");
                let sample = sample_format_string(&template);
                for language in [Language::French, Language::German, Language::Spanish] {
                    assert!(
                        TuiTranslations::for_language(language)
                            .try_dynamic(&sample)
                            .is_some(),
                        "uncatalogued central status in {name}: {template:?}"
                    );
                }
            }
        }
    }
}
