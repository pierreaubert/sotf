use super::Language;

pub struct AudioPreferencesPersistenceTranslations;

impl AudioPreferencesPersistenceTranslations {
    pub fn recovery_stopped(language: Language, stopped: bool) -> &'static str {
        let messages = match language {
            Language::French => [
                "Impossible de restaurer ou d’arrêter la lecture. Réessayez l’arrêt avant d’appliquer les réglages",
                "Impossible de restaurer la lecture précédente. La lecture est arrêtée ; réessayez Appliquer",
            ],
            Language::German => [
                "Wiedergabe konnte weder wiederhergestellt noch gestoppt werden. Vor dem Anwenden erneut stoppen",
                "Vorherige Wiedergabe konnte nicht wiederhergestellt werden. Wiedergabe gestoppt; erneut Anwenden wählen",
            ],
            Language::Spanish => [
                "No se pudo restaurar ni detener la reproducción. Intenta detenerla antes de aplicar los ajustes",
                "No se pudo restaurar la reproducción anterior. Reproducción detenida; vuelve a pulsar Aplicar",
            ],
            _ => [
                "Could not restore or stop playback. Retry Stop before applying settings",
                "Could not restore previous playback. Playback stopped; retry Apply",
            ],
        };
        let message = messages[usize::from(stopped)];
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn systemwide_format_labels(language: Language) -> [&'static str; 3] {
        let labels = match language {
            Language::French => [
                "Fréquence d’échantillonnage système",
                "Canaux système",
                "Taille du tampon système",
            ],
            Language::German => [
                "Systemweite Abtastrate",
                "Systemweite Kanäle",
                "Systemweite Puffergröße",
            ],
            Language::Spanish => [
                "Frecuencia de muestreo del sistema",
                "Canales del sistema",
                "Tamaño del búfer del sistema",
            ],
            _ => [
                "Systemwide sample rate",
                "Systemwide channels",
                "Systemwide buffer size",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }

    pub fn inactive_systemwide_format(language: Language) -> &'static str {
        let message = match language {
            Language::French => {
                "Ces réglages de format s’appliquent uniquement à l’entrée système. Le lecteur de fichiers reste sélectionné."
            }
            Language::German => {
                "Diese Formateinstellungen gelten nur für den systemweiten Eingang. Der Dateiplayer bleibt ausgewählt."
            }
            Language::Spanish => {
                "Estos ajustes de formato solo se aplican a la entrada del sistema. El reproductor de archivos sigue seleccionado."
            }
            _ => {
                "These format settings apply only to systemwide input. File player remains selected."
            }
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn system_default(language: Language) -> &'static str {
        let message = match language {
            Language::French => "Sortie par défaut du système",
            Language::German => "Systemstandardausgabe",
            Language::Spanish => "Salida predeterminada del sistema",
            _ => "System default output",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn unnamed_device(language: Language) -> &'static str {
        let message = match language {
            Language::French => "Périphérique sans nom",
            Language::German => "Unbenanntes Gerät",
            Language::Spanish => "Dispositivo sin nombre",
            _ => "Unnamed device",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn device_details(language: Language) -> &'static str {
        let message = match language {
            Language::French => "Détails des périphériques",
            Language::German => "Gerätedetails",
            Language::Spanish => "Detalles de los dispositivos",
            _ => "Device details",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn applying(language: Language) -> &'static str {
        let message = match language {
            Language::French => "Application des réglages audio…",
            Language::German => "Audioeinstellungen werden angewendet…",
            Language::Spanish => "Aplicando los ajustes de audio…",
            _ => "Applying audio settings…",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn save_failed(language: Language, active: bool) -> &'static str {
        let messages = match language {
            Language::French => [
                "Impossible d’enregistrer les réglages audio. Les réglages précédents sont conservés",
                "Impossible d’enregistrer les réglages audio. Réessayez Appliquer",
            ],
            Language::German => [
                "Audioeinstellungen konnten nicht gespeichert werden. Die bisherigen Einstellungen bleiben erhalten",
                "Audioeinstellungen konnten nicht gespeichert werden. Erneut Anwenden wählen",
            ],
            Language::Spanish => [
                "No se pudieron guardar los ajustes de audio. Se conservan los ajustes anteriores",
                "No se pudieron guardar los ajustes de audio. Vuelve a pulsar Aplicar",
            ],
            _ => [
                "Could not save audio settings. The previous settings are retained",
                "Could not save audio settings. Retry Apply",
            ],
        };
        let message = messages[usize::from(active)];
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }
}

pub struct PreferencesCloseTranslations;
impl PreferencesCloseTranslations {
    pub fn for_language(language: Language) -> [&'static str; 4] {
        let labels = match language {
            Language::French => [
                "Appliquer les modifications audio avant de fermer ?",
                "Continuer la modification",
                "Ignorer les modifications",
                "Appliquer et fermer",
            ],
            Language::German => [
                "Audioänderungen vor dem Schließen anwenden?",
                "Weiter bearbeiten",
                "Änderungen verwerfen",
                "Anwenden und schließen",
            ],
            Language::Spanish => [
                "¿Aplicar los cambios de audio antes de cerrar?",
                "Seguir editando",
                "Descartar cambios",
                "Aplicar y cerrar",
            ],
            _ => [
                "Apply audio changes before closing?",
                "Keep editing",
                "Discard changes",
                "Apply and close",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct PlaylistCompositionTranslations;
impl PlaylistCompositionTranslations {
    pub fn for_language(language: Language) -> [&'static str; 2] {
        let labels = match language {
            Language::French => [
                "Ajouter la piste actuelle",
                "Cette liste est vide. Ajoutez la piste actuelle pour commencer.",
            ],
            Language::German => [
                "Aktuellen Titel hinzufügen",
                "Diese Wiedergabeliste ist leer. Füge zunächst den aktuellen Titel hinzu.",
            ],
            Language::Spanish => [
                "Añadir pista actual",
                "Esta lista está vacía. Añade la pista actual para empezar.",
            ],
            _ => [
                "Add current track",
                "This playlist is empty. Add the current track to get started.",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct FlatQueueTranslations;
impl FlatQueueTranslations {
    pub fn for_language(language: Language) -> [&'static str; 9] {
        let labels = match language {
            Language::French => [
                "À suivre",
                "Les modifications ci-dessous concernent les pistes à venir.",
                "Lecture",
                "En pause",
                "Monter",
                "Descendre",
                "Supprimer",
                "Parcourir la bibliothèque",
                "Rien à suivre",
            ],
            Language::German => [
                "Als Nächstes",
                "Änderungen unten betreffen kommende Titel.",
                "Wiedergabe",
                "Pausiert",
                "Nach oben",
                "Nach unten",
                "Entfernen",
                "Bibliothek durchsuchen",
                "Keine weiteren Titel",
            ],
            Language::Spanish => [
                "A continuación",
                "Los cambios siguientes afectan a las próximas pistas.",
                "Reproduciendo",
                "En pausa",
                "Subir",
                "Bajar",
                "Eliminar",
                "Explorar biblioteca",
                "No hay más pistas",
            ],
            _ => [
                "Up next",
                "Changes below affect upcoming tracks.",
                "Playing",
                "Paused",
                "Move up",
                "Move down",
                "Remove",
                "Browse library",
                "Nothing up next",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct LibraryToolbarTranslations;
impl LibraryToolbarTranslations {
    pub fn for_language(language: Language) -> [&'static str; 10] {
        let labels = match language {
            Language::French => [
                "Bibliothèque",
                "Filtres",
                "Favoris uniquement",
                "Aucun résultat. Modifiez la recherche ou les filtres.",
                "Vue en grille",
                "Vue en liste",
                "Trier par",
                "Récent",
                "Titre",
                "Artiste",
            ],
            Language::German => [
                "Bibliothek",
                "Filter",
                "Nur Favoriten",
                "Keine Treffer. Suche oder Filter ändern.",
                "Rasteransicht",
                "Listenansicht",
                "Sortieren nach",
                "Neueste",
                "Titel",
                "Interpret",
            ],
            Language::Spanish => [
                "Biblioteca",
                "Filtros",
                "Solo favoritos",
                "Sin resultados. Cambia la búsqueda o los filtros.",
                "Vista de cuadrícula",
                "Vista de lista",
                "Ordenar por",
                "Reciente",
                "Título",
                "Artista",
            ],
            _ => [
                "Library",
                "Filters",
                "Favorites only",
                "No matches. Try another search or change your filters.",
                "Grid view",
                "List view",
                "Sort by",
                "Recent",
                "Title",
                "Artist",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct NowPlayingTranslations;
impl NowPlayingTranslations {
    pub fn for_language(language: Language) -> [&'static str; 14] {
        let labels = match language {
            Language::French => [
                "Chemin du signal",
                "Informations sur la piste",
                "Voir la station",
                "Flux en direct · navigation indisponible",
                "Flux",
                "Ouvrir le traitement",
                "Indisponible",
                "Traitement contourné",
                "Rééchantillonnage",
                "Latence (échantillons)",
                "Canaux",
                "Bits par échantillon",
                "Source",
                "Sortie",
            ],
            Language::German => [
                "Signalweg",
                "Titelinformationen",
                "Sender anzeigen",
                "Live-Stream · kein Suchlauf",
                "Stream",
                "Verarbeitung öffnen",
                "Nicht verfügbar",
                "Verarbeitung umgangen",
                "Abtastratenwandlung",
                "Latenz (Samples)",
                "Kanäle",
                "Bits pro Sample",
                "Quelle",
                "Ausgabe",
            ],
            Language::Spanish => [
                "Ruta de señal",
                "Información de la pista",
                "Ver emisora",
                "Emisión en directo · sin desplazamiento",
                "Emisión",
                "Abrir procesamiento",
                "No disponible",
                "Procesamiento omitido",
                "Remuestreo",
                "Latencia (muestras)",
                "Canales",
                "Bits por muestra",
                "Fuente",
                "Salida",
            ],
            _ => [
                "Signal path",
                "Track information",
                "View station",
                "Live stream · seeking unavailable",
                "Stream",
                "Open processing",
                "Unavailable",
                "Processing bypassed",
                "Resampling",
                "Latency (samples)",
                "Channels",
                "Bits per sample",
                "Source",
                "Output",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct HomeShelfTranslations;
impl HomeShelfTranslations {
    pub fn for_language(language: Language) -> [&'static str; 3] {
        let labels = match language {
            Language::French => ["Écoutés récemment", "Favoris", "Parcourir la bibliothèque"],
            Language::German => ["Zuletzt gehört", "Favoriten", "Bibliothek durchsuchen"],
            Language::Spanish => [
                "Escuchados recientemente",
                "Favoritos",
                "Explorar biblioteca",
            ],
            _ => ["Recently played", "Favorites", "Browse library"],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct HomeEntryTranslations;

impl HomeEntryTranslations {
    pub fn for_language(language: Language) -> [&'static str; 5] {
        let labels = match language {
            Language::French => [
                "Continuer l’écoute",
                "Reprendre",
                "Ajouter un dossier musical",
                "Connecter une bibliothèque",
                "En cours de lecture",
            ],
            Language::German => [
                "Weiterhören",
                "Fortsetzen",
                "Musikordner hinzufügen",
                "Bibliothek verbinden",
                "Aktuelle Wiedergabe",
            ],
            Language::Spanish => [
                "Seguir escuchando",
                "Reanudar",
                "Añadir carpeta de música",
                "Conectar biblioteca",
                "Reproduciendo",
            ],
            _ => [
                "Continue listening",
                "Resume",
                "Add music folder",
                "Connect library",
                "Now playing",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

pub struct HeadphoneAuditionTranslations {
    pub original: &'static str,
    pub corrected: &'static str,
    pub stop: &'static str,
    pub preamp: &'static str,
    pub headroom: &'static str,
    pub prepare: &'static str,
    pub paused: &'static str,
    pub pending: &'static str,
}

impl HeadphoneAuditionTranslations {
    pub fn for_language(language: Language) -> Self {
        let labels = match language {
            Language::French => [
                "Original",
                "Corrigé",
                "Arrêter l’aperçu",
                "Préampli",
                "Atténuation calculée",
                "Choisissez Original ou Corrigé pour préparer l’écoute avec la même atténuation.",
                "Utilisez les commandes de lecture pour écouter.",
                "Application de l’aperçu…",
            ],
            Language::German => [
                "Original",
                "Korrigiert",
                "Vorschau beenden",
                "Vorverstärkung",
                "Berechnete Absenkung",
                "Original oder Korrigiert wählen, um mit gleicher Absenkung zu hören.",
                "Zum Anhören die Wiedergabesteuerung verwenden.",
                "Vorschau wird angewendet…",
            ],
            Language::Spanish => [
                "Original",
                "Corregido",
                "Detener escucha",
                "Preamplificador",
                "Atenuación calculada",
                "Elija Original o Corregido para escuchar con la misma atenuación.",
                "Use los controles de reproducción para escuchar.",
                "Aplicando escucha…",
            ],
            _ => [
                "Original",
                "Corrected",
                "Stop preview",
                "Preamp",
                "Calculated attenuation",
                "Choose Original or Corrected to prepare listening with the same attenuation.",
                "Use the playback controls to listen.",
                "Applying preview…",
            ],
        };
        let labels = if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        };
        Self {
            original: labels[0],
            corrected: labels[1],
            stop: labels[2],
            preamp: labels[3],
            headroom: labels[4],
            prepare: labels[5],
            paused: labels[6],
            pending: labels[7],
        }
    }
}

pub struct TakeReviewTranslations {
    pub title: &'static str,
    pub accept: &'static str,
    pub retake: &'static str,
    pub accepted: &'static str,
    pub unreviewed: &'static str,
    pub missing: &'static str,
    pub microphone: &'static str,
    pub position: &'static str,
    pub saved: &'static str,
    pub unknown: &'static str,
    pub imported_routing_required: &'static str,
    pub output: &'static str,
}

impl TakeReviewTranslations {
    pub fn dimension_labels(language: Language) -> [&'static str; 3] {
        let labels = match language {
            Language::French => ["Largeur", "Profondeur", "Hauteur"],
            Language::German => ["Breite", "Tiefe", "Höhe"],
            Language::Spanish => ["Ancho", "Profundidad", "Altura"],
            _ => ["Width", "Depth", "Height"],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }

    pub fn dimensions_warning(language: Language) -> &'static str {
        let message = match language {
            Language::French => {
                "Renseignez les trois dimensions positives, ou mettez-les toutes à zéro pour les omettre."
            }
            Language::German => {
                "Geben Sie alle drei Maße positiv an oder setzen Sie alle auf null, um sie wegzulassen."
            }
            Language::Spanish => {
                "Introduzca las tres dimensiones positivas o ponga todas a cero para omitirlas."
            }
            _ => "Enter all three positive dimensions, or set all three to zero to omit them.",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn references_help(language: Language) -> &'static str {
        let message = match language {
            Language::French => {
                "Les réponses mesurées sont incluses dans recordings.json. Les fichiers WAV/CSV référencés restent à leurs emplacements actuels."
            }
            Language::German => {
                "Die Messkurven sind in recordings.json enthalten. Referenzierte WAV-/CSV-Dateien bleiben an ihren bisherigen Speicherorten."
            }
            Language::Spanish => {
                "Las respuestas medidas se incluyen en recordings.json. Los archivos WAV/CSV referenciados permanecen en sus ubicaciones actuales."
            }
            _ => {
                "Measured responses are included in recordings.json. Referenced WAV/CSV files remain at their existing paths."
            }
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn save_status(
        language: Language,
        status: sotf_audio_player::ui_models::take_review::RecordingSaveStatus,
    ) -> &'static str {
        use sotf_audio_player::ui_models::take_review::RecordingSaveStatus;
        let messages = match language {
            Language::French => [
                "Cette session n’a pas encore été enregistrée.",
                "La session actuelle a été enregistrée.",
                "Le dernier enregistrement ne contient pas vos dernières modifications.",
            ],
            Language::German => [
                "Diese Sitzung wurde noch nicht gespeichert.",
                "Die aktuelle Sitzung wurde gespeichert.",
                "Die letzte Speicherung enthält Ihre neuesten Änderungen noch nicht.",
            ],
            Language::Spanish => [
                "Esta sesión aún no se ha guardado.",
                "Se ha guardado la sesión actual.",
                "El último guardado no incluye los cambios más recientes.",
            ],
            _ => [
                "This session has not been saved yet.",
                "The current session has been saved.",
                "The last save does not include your latest changes.",
            ],
        };
        let message = messages[match status {
            RecordingSaveStatus::NotSaved => 0,
            RecordingSaveStatus::Current => 1,
            RecordingSaveStatus::Previous => 2,
        }];
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }

    pub fn for_language(language: Language) -> Self {
        let labels = match language {
            Language::French => [
                "Vérifier chaque prise",
                "Accepter la prise",
                "Refaire la prise",
                "Acceptée",
                "À vérifier",
                "Manquante",
                "Microphone",
                "Position",
                "Enregistré",
                "Inconnu",
                "Choisissez la sortie et le microphone de chaque prise importée, puis refaites-la individuellement.",
                "Sortie",
            ],
            Language::German => [
                "Jede Aufnahme prüfen",
                "Aufnahme akzeptieren",
                "Erneut aufnehmen",
                "Akzeptiert",
                "Zu prüfen",
                "Fehlt",
                "Mikrofon",
                "Position",
                "Gespeichert",
                "Unbekannt",
                "Wählen Sie Ausgang und Mikrofon jeder importierten Messung und nehmen Sie sie einzeln erneut auf.",
                "Ausgang",
            ],
            Language::Spanish => [
                "Revisar cada toma",
                "Aceptar toma",
                "Repetir toma",
                "Aceptada",
                "Sin revisar",
                "Falta",
                "Micrófono",
                "Posición",
                "Guardado",
                "Desconocido",
                "Elija la salida y el micrófono de cada toma importada y repítala individualmente.",
                "Salida",
            ],
            _ => [
                "Review every take",
                "Accept take",
                "Retake",
                "Accepted",
                "Needs review",
                "Missing",
                "Microphone",
                "Position",
                "Saved",
                "Unknown",
                "Choose the output and microphone for each imported take, then retake it individually.",
                "Output",
            ],
        };
        let labels = if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        };
        Self {
            title: labels[0],
            accept: labels[1],
            retake: labels[2],
            accepted: labels[3],
            unreviewed: labels[4],
            missing: labels[5],
            microphone: labels[6],
            position: labels[7],
            saved: labels[8],
            unknown: labels[9],
            imported_routing_required: labels[10],
            output: labels[11],
        }
    }
}

pub struct DesktopTranslations {
    pub preferences: &'static str,
    pub search: &'static str,
    pub category: &'static str,
    pub done: &'static str,
    pub no_results: &'static str,
    pub spectrum_live: &'static str,
    pub spectrum_held: &'static str,
    pub spectrum_unavailable: &'static str,
    pub stale_result: &'static str,
    pub apply_routing: &'static str,
    pub discard_routing: &'static str,
    pub routing_draft: &'static str,
    pub routing_connections: &'static str,
    pub studio_sections: [&'static str; 4],
    pub apply_audio: &'static str,
    pub output_unavailable: &'static str,
    pub output_failed: &'static str,
    pub recording_incomplete: &'static str,
    pub view_album: &'static str,
    pub categories: [&'static str; 7],
}

impl DesktopTranslations {
    pub fn routing_form(language: Language) -> [&'static str; 3] {
        let labels = match language {
            Language::French => ["De", "Vers", "Connecter"],
            Language::German => ["Von", "Nach", "Verbinden"],
            Language::Spanish => ["Desde", "Hasta", "Conectar"],
            _ => ["From", "To", "Connect"],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }

    pub fn matrix_routes(language: Language) -> [&'static str; 2] {
        let labels = match language {
            Language::French => ["Source", "Destination"],
            Language::German => ["Quelle", "Ziel"],
            Language::Spanish => ["Origen", "Destino"],
            _ => ["Source", "Destination"],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }

    pub fn typography_settings(language: Language) -> [&'static str; 3] {
        let labels = match language {
            Language::French => [
                "Taille du texte",
                "Taille minimale de police",
                "Taille maximale de police",
            ],
            Language::German => [
                "Textgröße",
                "Minimale Schriftgröße",
                "Maximale Schriftgröße",
            ],
            Language::Spanish => [
                "Tamaño del texto",
                "Tamaño mínimo de fuente",
                "Tamaño máximo de fuente",
            ],
            _ => ["Text size", "Minimum font size", "Maximum font size"],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }

    pub fn for_language(language: Language) -> Self {
        match language {
            Language::French => Self {
                preferences: "Préférences",
                search: "Rechercher dans les préférences",
                category: "Catégorie",
                done: "Terminé",
                no_results: "Aucun réglage correspondant",
                spectrum_live: "En direct",
                spectrum_held: "Spectre figé",
                spectrum_unavailable: "Indisponible",
                apply_routing: "Appliquer le routage",
                discard_routing: "Annuler les modifications",
                routing_draft: "Brouillon · modifications après application",
                routing_connections: "Connexions",
                apply_audio: "Appliquer les réglages audio",
                output_unavailable: "La sortie choisie n’est plus disponible.",
                recording_incomplete: "Capturez et acceptez chaque canal et position avant d’enregistrer.",
                view_album: "Voir l’album",
                output_failed: "Échec de l’application des réglages audio. Les réglages précédents restent actifs.",
                studio_sections: ["Traitement", "Mesure", "Correction", "Tests d’écoute"],
                stale_result: "Les entrées ont changé. Relancez l’optimisation avant d’appliquer ou d’exporter.",
                categories: [
                    "Audio et lecture",
                    "Bibliothèque et métadonnées",
                    "Apparence et langue",
                    "Raccourcis clavier",
                    "Plugins",
                    "Connexions",
                    "Avancé",
                ],
            },
            Language::German => Self {
                preferences: "Einstellungen",
                search: "Einstellungen durchsuchen",
                category: "Kategorie",
                done: "Fertig",
                no_results: "Keine passenden Einstellungen",
                spectrum_live: "Live",
                spectrum_held: "Angehaltenes Spektrum",
                spectrum_unavailable: "Nicht verfügbar",
                apply_routing: "Routing anwenden",
                discard_routing: "Änderungen verwerfen",
                routing_draft: "Entwurf · Änderungen nach Anwenden",
                routing_connections: "Verbindungen",
                apply_audio: "Audioeinstellungen anwenden",
                output_unavailable: "Die gewählte Ausgabe ist nicht mehr verfügbar.",
                recording_incomplete: "Vor dem Speichern jeden Kanal und jede Position aufnehmen und akzeptieren.",
                view_album: "Album anzeigen",
                output_failed: "Audioeinstellungen konnten nicht angewendet werden. Die bisherigen Einstellungen bleiben aktiv.",
                studio_sections: ["Verarbeitung", "Messen", "Kalibrieren", "Hörtests"],
                stale_result: "Die Eingaben wurden geändert. Vor Anwenden oder Exportieren erneut optimieren.",
                categories: [
                    "Audio und Wiedergabe",
                    "Bibliothek und Metadaten",
                    "Darstellung und Sprache",
                    "Tastenkürzel",
                    "Plugins",
                    "Verbindungen",
                    "Erweitert",
                ],
            },
            Language::Spanish => Self {
                preferences: "Preferencias",
                search: "Buscar en preferencias",
                category: "Categoría",
                done: "Listo",
                no_results: "No hay ajustes coincidentes",
                spectrum_live: "En directo",
                spectrum_held: "Espectro retenido",
                spectrum_unavailable: "No disponible",
                apply_routing: "Aplicar enrutamiento",
                discard_routing: "Descartar cambios",
                routing_draft: "Borrador · cambios al aplicar",
                routing_connections: "Conexiones",
                apply_audio: "Aplicar ajustes de audio",
                output_unavailable: "La salida seleccionada ya no está disponible.",
                recording_incomplete: "Capture y acepte cada canal y posición antes de guardar.",
                view_album: "Ver álbum",
                output_failed: "No se pudieron aplicar los ajustes de audio. Los ajustes anteriores siguen activos.",
                studio_sections: [
                    "Procesamiento",
                    "Medición",
                    "Calibración",
                    "Pruebas de escucha",
                ],
                stale_result: "Las entradas han cambiado. Optimice de nuevo antes de aplicar o exportar.",
                categories: [
                    "Audio y reproducción",
                    "Biblioteca y metadatos",
                    "Apariencia e idioma",
                    "Atajos de teclado",
                    "Plugins",
                    "Conexiones",
                    "Avanzado",
                ],
            },
            Language::Pseudo => {
                let mut text = Self::for_language(Language::English);
                text.preferences = super::pseudo_static(text.preferences);
                text.search = super::pseudo_static(text.search);
                text.category = super::pseudo_static(text.category);
                text.done = super::pseudo_static(text.done);
                text.no_results = super::pseudo_static(text.no_results);
                text.spectrum_live = super::pseudo_static(text.spectrum_live);
                text.spectrum_held = super::pseudo_static(text.spectrum_held);
                text.spectrum_unavailable = super::pseudo_static(text.spectrum_unavailable);
                text.stale_result = super::pseudo_static(text.stale_result);
                text.apply_routing = super::pseudo_static(text.apply_routing);
                text.discard_routing = super::pseudo_static(text.discard_routing);
                text.routing_draft = super::pseudo_static(text.routing_draft);
                text.routing_connections = super::pseudo_static(text.routing_connections);
                text.studio_sections = text.studio_sections.map(super::pseudo_static);
                text.apply_audio = super::pseudo_static(text.apply_audio);
                text.output_unavailable = super::pseudo_static(text.output_unavailable);
                text.recording_incomplete = super::pseudo_static(text.recording_incomplete);
                text.view_album = super::pseudo_static(text.view_album);
                text.output_failed = super::pseudo_static(text.output_failed);
                text.categories = text.categories.map(super::pseudo_static);
                text
            }
            Language::English => Self {
                preferences: "Preferences",
                search: "Search preferences",
                category: "Category",
                done: "Done",
                no_results: "No matching settings",
                spectrum_live: "Live",
                spectrum_held: "Held spectrum",
                spectrum_unavailable: "Unavailable",
                apply_routing: "Apply routing",
                discard_routing: "Discard changes",
                routing_draft: "Draft · changes take effect on Apply",
                routing_connections: "Connections",
                apply_audio: "Apply audio settings",
                output_unavailable: "The selected output is no longer available.",
                recording_incomplete: "Capture and accept every channel and position before saving.",
                view_album: "View album",
                output_failed: "Could not apply audio settings. The previous settings remain active.",
                studio_sections: ["Processing", "Measure", "Calibrate", "Listening tests"],
                stale_result: "Inputs have changed. Run optimization again before applying or exporting.",
                categories: [
                    "Audio & playback",
                    "Library & metadata",
                    "Appearance & language",
                    "Keyboard shortcuts",
                    "Plugins",
                    "Connections",
                    "Advanced",
                ],
            },
        }
    }
}

/// Navigation and contextual guidance for the shared AutoEQ decisions.
pub struct AutoEqStageTranslations {
    pub mode_labels: [&'static str; 4],
    pub mode_descriptions: [&'static str; 4],
    pub stages: [&'static str; 5],
    pub enabled: &'static str,
    pub disabled: &'static str,
    pub no_timing: &'static str,
    pub review_hint: &'static str,
}

impl AutoEqStageTranslations {
    pub fn for_language(language: Language) -> Self {
        let labels = match language {
            Language::French => [
                "Objectifs",
                "Conception des filtres",
                "Temps et mesures",
                "Algorithme",
                "Vérifier la configuration",
                "Les réglages temporels et multi-mesures ne sont pas disponibles dans ce parcours.",
                "Cette configuration appartient au parcours en cours. Lancez l’optimisation depuis ce parcours.",
            ],
            Language::German => [
                "Ziele",
                "Filterentwurf",
                "Zeit und Messungen",
                "Algorithmus",
                "Konfiguration prüfen",
                "Zeitkorrektur und mehrere Messungen sind in diesem Ablauf nicht verfügbar.",
                "Diese Konfiguration gehört zum aktuellen Ablauf. Starten Sie dort die Optimierung.",
            ],
            Language::Spanish => [
                "Objetivos",
                "Diseño de filtros",
                "Tiempo y mediciones",
                "Algoritmo",
                "Revisar configuración",
                "Los ajustes temporales y de múltiples mediciones no están disponibles en este flujo.",
                "Esta configuración pertenece al flujo actual. Inicie la optimización desde ese flujo.",
            ],
            _ => [
                "Goals",
                "Filter design",
                "Timing & measurements",
                "Algorithm",
                "Review configuration",
                "Timing and multiple-measurement settings are not available in this workflow.",
                "This configuration belongs to the current workflow. Start optimization from that workflow.",
            ],
        };
        let labels = if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        };
        let (enabled, disabled) = match language {
            Language::French => ("Activé", "Désactivé"),
            Language::German => ("Aktiviert", "Deaktiviert"),
            Language::Spanish => ("Activado", "Desactivado"),
            _ => ("Enabled", "Disabled"),
        };
        Self {
            enabled: if language == Language::Pseudo {
                super::pseudo_static(enabled)
            } else {
                enabled
            },
            disabled: if language == Language::Pseudo {
                super::pseudo_static(disabled)
            } else {
                disabled
            },
            mode_labels: (match language {
                Language::French => [
                    "IIR (recommandé)",
                    "FIR",
                    "Mixte",
                    "Phase mixte (recommandé)",
                ],
                Language::German => [
                    "IIR (empfohlen)",
                    "FIR",
                    "Gemischt",
                    "Gemischte Phase (empfohlen)",
                ],
                Language::Spanish => [
                    "IIR (recomendado)",
                    "FIR",
                    "Mixto",
                    "Fase mixta (recomendado)",
                ],
                _ => [
                    "IIR (recommended)",
                    "FIR",
                    "Mixed",
                    "Mixed Phase (recommended)",
                ],
            })
            .map(|label| {
                if language == Language::Pseudo {
                    super::pseudo_static(label)
                } else {
                    label
                }
            }),
            mode_descriptions: (match language {
                Language::French => [
                    "IIR paramétrique uniquement",
                    "Mode FIR classique",
                    "Combinaison IIR et FIR",
                    "IIR et FIR sur la phase excédentaire uniquement",
                ],
                Language::German => [
                    "Nur parametrischer IIR",
                    "Klassischer FIR-Modus",
                    "IIR und FIR kombinieren",
                    "IIR und FIR nur für die Überschussphase",
                ],
                Language::Spanish => [
                    "Solo IIR paramétrico",
                    "Modo FIR clásico",
                    "Combinar IIR y FIR",
                    "IIR y FIR solo en la fase excedente",
                ],
                _ => [
                    "Parametric IIR only",
                    "Classical FIR mode",
                    "Mix IIR and FIR",
                    "Mix IIR and FIR on excess phase only",
                ],
            })
            .map(|label| {
                if language == Language::Pseudo {
                    super::pseudo_static(label)
                } else {
                    label
                }
            }),
            stages: [labels[0], labels[1], labels[2], labels[3], labels[4]],
            no_timing: labels[5],
            review_hint: labels[6],
        }
    }
}

/// Context and inspection labels for the fullscreen spectrum.
pub struct SpectrumInspectionTranslations {
    pub title: &'static str,
    pub output_rate: &'static str,
    pub nyquist: &'static str,
    pub rate_unknown: &'static str,
    pub inspect_hint: &'static str,
    pub units: &'static str,
    pub smoothing: &'static str,
    pub range: &'static str,
    pub bands: &'static str,
    pub frequency: &'static str,
}
impl SpectrumInspectionTranslations {
    pub fn for_language(language: Language) -> Self {
        let labels = match language {
            Language::French => [
                "Détails de l’analyse",
                "Fréquence de sortie",
                "Nyquist en sortie",
                "Fréquence de sortie indisponible",
                "Survolez le graphique ou choisissez une fréquence dans les détails.",
                "Les niveaux sont en dBFS, pas en dB SPL acoustiques calibrés.",
                "Le lissage d’affichage moyenne cinq bandes voisines sans modifier le son.",
                "Plage affichée",
                "Bandes",
                "Fréquence à examiner (Hz)",
            ],
            Language::German => [
                "Analysedetails",
                "Ausgaberate",
                "Ausgabe-Nyquist",
                "Ausgaberate nicht verfügbar",
                "Zeiger über das Diagramm bewegen oder eine Frequenz in den Details wählen.",
                "Pegel sind in dBFS angegeben, nicht als kalibrierter Schalldruckpegel.",
                "Die Anzeigeglättung mittelt fünf benachbarte Bänder und verändert den Klang nicht.",
                "Anzeigebereich",
                "Bänder",
                "Frequenz untersuchen (Hz)",
            ],
            Language::Spanish => [
                "Detalles del análisis",
                "Frecuencia de salida",
                "Nyquist de salida",
                "Frecuencia de salida no disponible",
                "Mueva el puntero sobre el gráfico o elija una frecuencia en los detalles.",
                "Los niveles están en dBFS, no en dB SPL acústicos calibrados.",
                "El suavizado visual promedia cinco bandas adyacentes sin modificar el audio.",
                "Rango mostrado",
                "Bandas",
                "Frecuencia a inspeccionar (Hz)",
            ],
            _ => [
                "Analysis details",
                "Output rate",
                "Output Nyquist",
                "Output rate unavailable",
                "Move the pointer over the plot or choose a frequency in Analysis details.",
                "Levels are in dBFS, not calibrated acoustic dB SPL.",
                "Display smoothing averages five adjacent bands without changing the audio.",
                "Displayed range",
                "Bands",
                "Inspect frequency (Hz)",
            ],
        };
        let labels = if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        };
        Self {
            title: labels[0],
            output_rate: labels[1],
            nyquist: labels[2],
            rate_unknown: labels[3],
            inspect_hint: labels[4],
            units: labels[5],
            smoothing: labels[6],
            range: labels[7],
            bands: labels[8],
            frequency: labels[9],
        }
    }
}

/// Album browsing actions, shared by the album header and track rows.
pub struct AlbumDetailTranslations;
impl AlbumDetailTranslations {
    pub fn for_language(language: Language) -> [&'static str; 9] {
        let labels = match language {
            Language::French => [
                "Retour à la bibliothèque",
                "Lire maintenant",
                "Lire ensuite",
                "Ajouter à la file",
                "Favori",
                "Dans les favoris",
                "Pistes",
                "Aucune piste disponible",
                "Ajouté à la file",
            ],
            Language::German => [
                "Zurück zur Bibliothek",
                "Jetzt abspielen",
                "Als Nächstes abspielen",
                "Zur Warteschlange",
                "Favorit",
                "Als Favorit gespeichert",
                "Titel",
                "Keine Titel verfügbar",
                "Zur Warteschlange hinzugefügt",
            ],
            Language::Spanish => [
                "Volver a la biblioteca",
                "Reproducir ahora",
                "Reproducir después",
                "Añadir a la cola",
                "Favorito",
                "En favoritos",
                "Pistas",
                "No hay pistas disponibles",
                "Añadido a la cola",
            ],
            _ => [
                "Back to Library",
                "Play now",
                "Play next",
                "Add to queue",
                "Favorite",
                "Favorited",
                "Tracks",
                "No tracks available",
                "Added to queue",
            ],
        };
        if language == Language::Pseudo {
            labels.map(super::pseudo_static)
        } else {
            labels
        }
    }
}

/// Copy for the capability preview; these choices do not install software updates.
#[derive(Clone, Copy)]
pub struct FeatureAvailabilityTranslations {
    pub preview: &'static str,
    pub levels: [&'static str; 3],
    pub screens: &'static str,
    pub plugins: &'static str,
    pub channel: &'static str,
    pub save_failed: &'static str,
    pub available: &'static str,
    pub unavailable: &'static str,
}

impl FeatureAvailabilityTranslations {
    pub fn save_error(self, error: impl std::fmt::Display) -> String {
        format!("{}: {error}", self.save_failed)
    }
    pub fn for_language(language: Language) -> Self {
        let copy = match language {
            Language::French => Self {
                preview: "Aperçu de la disponibilité",
                levels: [
                    "Fonctionnalités stables uniquement",
                    "Fonctionnalités stables et bêta",
                    "Fonctionnalités stables, bêta et alpha",
                ],
                screens: "Écrans",
                plugins: "Plugins",
                channel: "canal de fonctionnalités",
                save_failed: "Impossible d’enregistrer le niveau de fonctionnalités",
                available: "Disponible",
                unavailable: "Indisponible",
            },
            Language::German => Self {
                preview: "Verfügbarkeitsvorschau",
                levels: [
                    "Nur stabile Funktionen",
                    "Stabile und Beta-Funktionen",
                    "Stabile, Beta- und Alpha-Funktionen",
                ],
                screens: "Ansichten",
                plugins: "Plugins",
                channel: "Funktionskanal",
                save_failed: "Funktionsumfang konnte nicht gespeichert werden",
                available: "Verfügbar",
                unavailable: "Nicht verfügbar",
            },
            Language::Spanish => Self {
                preview: "Vista previa de disponibilidad",
                levels: [
                    "Solo funciones estables",
                    "Funciones estables y beta",
                    "Funciones estables, beta y alfa",
                ],
                screens: "Pantallas",
                plugins: "Plugins",
                channel: "canal de funciones",
                save_failed: "No se pudo guardar el nivel de funciones",
                available: "Disponible",
                unavailable: "No disponible",
            },
            Language::English | Language::Pseudo => Self {
                preview: "Availability preview",
                levels: [
                    "Stable features only",
                    "Stable and Beta features",
                    "Stable, Beta, and Alpha features",
                ],
                screens: "Screens",
                plugins: "Plugins",
                channel: "feature channel",
                save_failed: "Could not save the feature channel",
                available: "Available",
                unavailable: "Unavailable",
            },
        };
        if language == Language::Pseudo {
            Self {
                preview: super::pseudo_static(copy.preview),
                levels: copy.levels.map(super::pseudo_static),
                screens: super::pseudo_static(copy.screens),
                plugins: super::pseudo_static(copy.plugins),
                channel: super::pseudo_static(copy.channel),
                save_failed: super::pseudo_static(copy.save_failed),
                available: super::pseudo_static(copy.available),
                unavailable: super::pseudo_static(copy.unavailable),
            }
        } else {
            copy
        }
    }
    pub fn summary(self, channel: sotf_audio_player::ReleaseChannel) -> &'static str {
        self.levels[match channel {
            sotf_audio_player::ReleaseChannel::Prod => 0,
            sotf_audio_player::ReleaseChannel::Beta => 1,
            sotf_audio_player::ReleaseChannel::Alpha => 2,
        }]
    }
}

pub struct LibraryAnalysisTranslations;

impl LibraryAnalysisTranslations {
    pub fn title(language: Language) -> &'static str {
        let message = match language {
            Language::French => "Analyse et maintenance",
            Language::German => "Analyse und Wartung",
            Language::Spanish => "Análisis y mantenimiento",
            _ => "Analysis and maintenance",
        };
        if language == Language::Pseudo {
            super::pseudo_static(message)
        } else {
            message
        }
    }
}

#[derive(Clone, Copy)]
pub struct LibraryMaintenanceTranslations {
    pub review: &'static str,
    pub retry_refresh: &'static str,
    pub changed: &'static str,
    pub checking: &'static str,
    pub description: &'static str,
    pub confirm: &'static str,
    pub cancel: &'static str,
    pub missing: &'static str,
    pub unchecked: &'static str,
    pub removed: &'static str,
    pub unavailable: &'static str,
}
impl LibraryMaintenanceTranslations {
    pub fn for_language(language: Language) -> Self {
        let copy = match language {
            Language::French => Self {
                review: "Examiner les entrées obsolètes",
                retry_refresh: "Réessayer le chargement de la bibliothèque",
                changed: "La bibliothèque a changé. Réessayez le chargement.",
                checking: "Vérification…",
                description: "Examinez les fichiers locaux manquants avant de supprimer leurs entrées. Les fichiers audio ne sont jamais supprimés.",
                confirm: "Supprimer les entrées examinées",
                cancel: "Annuler",
                missing: "Fichiers manquants",
                unchecked: "Chemins non vérifiés",
                removed: "Entrées supprimées",
                unavailable: "La base de données est indisponible",
            },
            Language::German => Self {
                review: "Veraltete Einträge prüfen",
                retry_refresh: "Bibliothek erneut laden",
                changed: "Die Bibliothek wurde geändert. Laden Sie sie erneut.",
                checking: "Prüfung…",
                description: "Prüfen Sie fehlende lokale Dateien, bevor ihre Bibliothekseinträge entfernt werden. Audiodateien werden niemals gelöscht.",
                confirm: "Geprüfte Einträge entfernen",
                cancel: "Abbrechen",
                missing: "Fehlende Dateien",
                unchecked: "Nicht prüfbare Pfade",
                removed: "Entfernte Einträge",
                unavailable: "Die Bibliotheksdatenbank ist nicht verfügbar",
            },
            Language::Spanish => Self {
                review: "Revisar entradas obsoletas",
                retry_refresh: "Reintentar cargar la biblioteca",
                changed: "La biblioteca ha cambiado. Vuelva a cargarla.",
                checking: "Comprobando…",
                description: "Revise los archivos locales ausentes antes de eliminar sus entradas. Los archivos de audio nunca se eliminan.",
                confirm: "Eliminar entradas revisadas",
                cancel: "Cancelar",
                missing: "Archivos ausentes",
                unchecked: "Rutas que no se pudieron comprobar",
                removed: "Entradas eliminadas",
                unavailable: "La base de datos no está disponible",
            },
            _ => Self {
                review: "Review stale entries",
                retry_refresh: "Retry library refresh",
                changed: "The library changed. Retry the refresh.",
                checking: "Checking…",
                description: "Review missing local files before removing their library entries. Audio files are never deleted.",
                confirm: "Remove reviewed entries",
                cancel: "Cancel",
                missing: "Missing files",
                unchecked: "Paths that could not be checked",
                removed: "Removed entries",
                unavailable: "Library database is unavailable",
            },
        };
        if language == Language::Pseudo {
            Self {
                review: super::pseudo_static(copy.review),
                retry_refresh: super::pseudo_static(copy.retry_refresh),
                changed: super::pseudo_static(copy.changed),
                checking: super::pseudo_static(copy.checking),
                description: super::pseudo_static(copy.description),
                confirm: super::pseudo_static(copy.confirm),
                cancel: super::pseudo_static(copy.cancel),
                missing: super::pseudo_static(copy.missing),
                unchecked: super::pseudo_static(copy.unchecked),
                removed: super::pseudo_static(copy.removed),
                unavailable: super::pseudo_static(copy.unavailable),
            }
        } else {
            copy
        }
    }
}

#[derive(Clone, Copy)]
pub struct HeadphoneIdentityTranslations {
    pub title: &'static str,
    pub model: &'static str,
    pub source: &'static str,
    pub bounds: &'static str,
    pub target: &'static str,
    pub rig: &'static str,
    pub compensation: &'static str,
    pub unknown: &'static str,
    pub compatibility: &'static str,
}
impl HeadphoneIdentityTranslations {
    pub fn target_label(language: Language, target: &str, fallback: &'static str) -> &'static str {
        let label = match (language, target) {
            (Language::French, "flat") => "Plat",
            (Language::German, "flat") => "Linear",
            (Language::Spanish, "flat") => "Plano",
            (Language::French, "custom") => "Personnalisé (fichier)",
            (Language::German, "custom") => "Benutzerdefiniert (Datei)",
            (Language::Spanish, "custom") => "Personalizado (archivo)",
            _ => fallback,
        };
        if language == Language::Pseudo {
            super::pseudo_static(label)
        } else {
            label
        }
    }

    pub fn file_message(&self, detail: &str) -> String {
        format!("{}: {detail}", self.source)
    }

    pub fn for_language(language: Language) -> Self {
        let copy = match language {
            Language::French => Self {
                title: "Identité de la mesure et compatibilité de la cible",
                model: "Modèle ou variante",
                source: "Fichier de mesure",
                bounds: "Plage de fréquences mesurée",
                target: "Cible",
                rig: "Banc de mesure et échantillon",
                compensation: "Compensation de la mesure",
                unknown: "Non fourni",
                compatibility: "La compatibilité n’est pas vérifiée : confirmez que la cible convient au banc de mesure et à la compensation.",
            },
            Language::German => Self {
                title: "Messidentität und Zielkompatibilität",
                model: "Modell oder Variante",
                source: "Messdatei",
                bounds: "Gemessener Frequenzbereich",
                target: "Ziel",
                rig: "Messaufbau und Exemplar",
                compensation: "Messkompensation",
                unknown: "Nicht angegeben",
                compatibility: "Kompatibilität nicht geprüft: Prüfen Sie, ob das Ziel zum Messaufbau und zur Kompensation passt.",
            },
            Language::Spanish => Self {
                title: "Identidad de medición y compatibilidad del objetivo",
                model: "Modelo o variante",
                source: "Archivo de medición",
                bounds: "Rango de frecuencias medido",
                target: "Objetivo",
                rig: "Equipo de medición y muestra",
                compensation: "Compensación de la medición",
                unknown: "No proporcionado",
                compatibility: "Compatibilidad sin verificar: confirme que el objetivo corresponde al equipo y a la compensación de la medición.",
            },
            _ => Self {
                title: "Measurement identity and target compatibility",
                model: "Model or variant",
                source: "Measurement file",
                bounds: "Measured frequency bounds",
                target: "Target",
                rig: "Measurement rig and sample",
                compensation: "Measurement compensation",
                unknown: "Not supplied",
                compatibility: "Compatibility is unverified: confirm that the target matches the measurement rig and compensation.",
            },
        };
        if language == Language::Pseudo {
            Self {
                title: super::pseudo_static(copy.title),
                model: super::pseudo_static(copy.model),
                source: super::pseudo_static(copy.source),
                bounds: super::pseudo_static(copy.bounds),
                target: super::pseudo_static(copy.target),
                rig: super::pseudo_static(copy.rig),
                compensation: super::pseudo_static(copy.compensation),
                unknown: super::pseudo_static(copy.unknown),
                compatibility: super::pseudo_static(copy.compatibility),
            }
        } else {
            copy
        }
    }
}

/// Export history is independent of calculation and playback application.
#[derive(Clone, Copy)]
pub struct CorrectionDeliveryTranslations {
    pub title: &'static str,
    pub calculated: &'static str,
    pub not_exported: &'static str,
    pub current_export: &'static str,
    pub previous_export: &'static str,
}

impl CorrectionDeliveryTranslations {
    pub fn for_language(language: Language) -> Self {
        let copy = match language {
            Language::French => Self {
                title: "Dernier export réussi",
                calculated: "Résultat calculé pour les entrées actuelles.",
                not_exported: "Aucun résultat exporté dans cette session.",
                current_export: "Le résultat actuel a été exporté.",
                previous_export: "L’export correspond à un résultat précédent.",
            },
            Language::German => Self {
                title: "Letzter erfolgreicher Export",
                calculated: "Ergebnis für die aktuellen Eingaben berechnet.",
                not_exported: "In dieser Sitzung wurde noch kein Ergebnis exportiert.",
                current_export: "Das aktuelle Ergebnis wurde exportiert.",
                previous_export: "Der Export gehört zu einem früheren Ergebnis.",
            },
            Language::Spanish => Self {
                title: "Última exportación correcta",
                calculated: "Resultado calculado para las entradas actuales.",
                not_exported: "No se ha exportado ningún resultado en esta sesión.",
                current_export: "Se ha exportado el resultado actual.",
                previous_export: "La exportación corresponde a un resultado anterior.",
            },
            _ => Self {
                title: "Last successful export",
                calculated: "Result calculated for the current inputs.",
                not_exported: "No result has been exported in this session.",
                current_export: "The current result has been exported.",
                previous_export: "The export belongs to a previous result.",
            },
        };
        if language == Language::Pseudo {
            Self {
                title: super::pseudo_static(copy.title),
                calculated: super::pseudo_static(copy.calculated),
                not_exported: super::pseudo_static(copy.not_exported),
                current_export: super::pseudo_static(copy.current_export),
                previous_export: super::pseudo_static(copy.previous_export),
            }
        } else {
            copy
        }
    }
}

#[derive(Clone, Copy)]
pub struct CorrectionApplicationTranslations {
    pub title: &'static str,
    pub not_applied: &'static str,
    pub pending: &'static str,
    pub applied: &'static str,
    pub previous: &'static str,
    pub changed: &'static str,
    pub failed: &'static str,
}
impl CorrectionApplicationTranslations {
    pub fn for_language(language: Language) -> Self {
        let copy = match language {
            Language::French => Self {
                title: "Application au lecteur",
                not_applied: "Aucun résultat appliqué dans cette session.",
                pending: "Application en cours — en attente du lecteur.",
                applied: "Le lecteur a accepté le résultat actuel.",
                previous: "Le lecteur utilise un résultat précédent.",
                changed: "La chaîne a changé ou attend une confirmation. Réappliquez pour confirmer ce résultat.",
                failed: "L’application a échoué. Corrigez le problème puis réessayez.",
            },
            Language::German => Self {
                title: "Anwendung im Player",
                not_applied: "In dieser Sitzung wurde noch kein Ergebnis angewendet.",
                pending: "Wird angewendet — Bestätigung des Players ausstehend.",
                applied: "Der Player hat das aktuelle Ergebnis übernommen.",
                previous: "Der Player verwendet ein früheres Ergebnis.",
                changed: "Die Verarbeitung wurde geändert oder ist noch unbestätigt. Ergebnis erneut anwenden.",
                failed: "Anwenden fehlgeschlagen. Problem beheben und erneut versuchen.",
            },
            Language::Spanish => Self {
                title: "Aplicación al reproductor",
                not_applied: "No se ha aplicado ningún resultado en esta sesión.",
                pending: "Aplicando — esperando confirmación del reproductor.",
                applied: "El reproductor ha aceptado el resultado actual.",
                previous: "El reproductor utiliza un resultado anterior.",
                changed: "La cadena ha cambiado o espera confirmación. Vuelva a aplicar para confirmar este resultado.",
                failed: "No se pudo aplicar. Corrija el problema y vuelva a intentarlo.",
            },
            _ => Self {
                title: "Playback application",
                not_applied: "No result has been applied in this session.",
                pending: "Applying — waiting for the player.",
                applied: "The player accepted the current result.",
                previous: "The player is using a previous result.",
                changed: "Processing changed or awaits confirmation. Apply again to confirm this result.",
                failed: "Application failed. Resolve the error and try again.",
            },
        };
        if language == Language::Pseudo {
            Self {
                title: super::pseudo_static(copy.title),
                not_applied: super::pseudo_static(copy.not_applied),
                pending: super::pseudo_static(copy.pending),
                applied: super::pseudo_static(copy.applied),
                previous: super::pseudo_static(copy.previous),
                changed: super::pseudo_static(copy.changed),
                failed: super::pseudo_static(copy.failed),
            }
        } else {
            copy
        }
    }
}

/// Presentation copy for guided ear-training course cards.
pub struct EarTrainingCourseTranslations {
    pub titles: [&'static str; 5],
    pub bands: &'static str,
    pub trials: &'static str,
    pub completed: &'static str,
}

impl EarTrainingCourseTranslations {
    pub fn change(language: Language, mode: sotf_audio_player::EqChangeMode) -> &'static str {
        use sotf_audio_player::EqChangeMode;
        let labels = match language {
            Language::French => [
                "Amplifications",
                "Atténuations",
                "Amplifications/atténuations",
            ],
            Language::German => ["Anhebungen", "Absenkungen", "Anhebungen/Absenkungen"],
            Language::Spanish => ["Realces", "Atenuaciones", "Realces/atenuaciones"],
            _ => ["Boosts", "Cuts", "Boosts/cuts"],
        };
        let label = labels[match mode {
            EqChangeMode::Boost => 0,
            EqChangeMode::Cut => 1,
            EqChangeMode::Mixed => 2,
        }];
        if language.is_pseudo() {
            super::pseudo_static(label)
        } else {
            label
        }
    }

    pub fn new(language: Language) -> Self {
        let (titles, labels) = match language {
            Language::French => (
                [
                    "Fondamentaux",
                    "Régions fréquentielles",
                    "Reconnaître les atténuations",
                    "Bandes fines",
                    "Maîtrise",
                ],
                ["bandes", "questions", "sessions terminées"],
            ),
            Language::German => (
                [
                    "Grundlagen",
                    "Frequenzbereiche",
                    "Absenkungen erkennen",
                    "Schmale Bänder",
                    "Meisterschaft",
                ],
                ["Bänder", "Fragen", "abgeschlossene Sitzungen"],
            ),
            Language::Spanish => (
                [
                    "Fundamentos",
                    "Regiones de frecuencia",
                    "Reconocer atenuaciones",
                    "Bandas estrechas",
                    "Dominio",
                ],
                ["bandas", "preguntas", "sesiones completadas"],
            ),
            _ => (
                [
                    "Foundations",
                    "Frequency regions",
                    "Hearing cuts",
                    "Fine bands",
                    "Mastery",
                ],
                ["bands", "questions", "sessions completed"],
            ),
        };
        let localize = |text| {
            if language.is_pseudo() {
                super::pseudo_static(text)
            } else {
                text
            }
        };
        Self {
            titles: titles.map(localize),
            bands: localize(labels[0]),
            trials: localize(labels[1]),
            completed: localize(labels[2]),
        }
    }

    pub fn title(&self, course: sotf_audio_player::EarTrainingCourse) -> &'static str {
        use sotf_audio_player::EarTrainingCourse;
        self.titles[match course {
            EarTrainingCourse::Foundations => 0,
            EarTrainingCourse::FrequencyRegions => 1,
            EarTrainingCourse::Cuts => 2,
            EarTrainingCourse::FineBands => 3,
            EarTrainingCourse::Mastery => 4,
        }]
    }
}

/// Localized history labels; missing legacy difficulty is explicit.
pub struct EarTrainingHistoryTranslations {
    pub heading: &'static str,
    pub empty: &'static str,
    pub unavailable: &'static str,
    pub compare: &'static str,
    pub exercises: [&'static str; 3],
    pub sessions: &'static str,
    pub accuracy: &'static str,
    pub streak: &'static str,
}
impl EarTrainingHistoryTranslations {
    pub fn recommendation(
        language: Language,
        recommendation: sotf_audio_player::ear_training::EarTrainingRecommendation,
    ) -> String {
        use sotf_audio_player::ear_training::EarTrainingRecommendation;
        let labels = match language {
            Language::French => [
                "Commencez par les Fondamentaux à 12 dB.",
                "Essayez une session d’identification des amplifications et atténuations.",
                "Concentrez-vous autour de {frequency} Hz, puis réduisez le gain de 3 dB.",
            ],
            Language::German => [
                "Beginnen Sie mit Grundlagen bei 12 dB.",
                "Versuchen Sie eine Sitzung zum Erkennen von Anhebungen und Absenkungen.",
                "Konzentrieren Sie sich auf {frequency} Hz und reduzieren Sie dann die Verstärkung um 3 dB.",
            ],
            Language::Spanish => [
                "Comience con Fundamentos a 12 dB.",
                "Pruebe una sesión de identificación de realces y atenuaciones.",
                "Concéntrese alrededor de {frequency} Hz y luego reduzca la ganancia en 3 dB.",
            ],
            _ => [
                "Start with Foundations at 12 dB.",
                "Try a boost/cut identification session.",
                "Focus around {frequency} Hz, then reduce gain by 3 dB.",
            ],
        };
        let index = match recommendation {
            EarTrainingRecommendation::Foundations => 0,
            EarTrainingRecommendation::BoostCut => 1,
            EarTrainingRecommendation::FocusFrequency { .. } => 2,
        };
        let text = if language.is_pseudo() {
            super::pseudo_static(labels[index])
        } else {
            labels[index]
        };
        match recommendation {
            EarTrainingRecommendation::FocusFrequency { frequency_hz } => {
                text.replace("{frequency}", &format!("{frequency_hz:.0}"))
            }
            _ => text.to_owned(),
        }
    }

    pub fn new(language: Language) -> Self {
        let (labels, exercises) = match language {
            Language::French => (
                [
                    "Historique des sessions",
                    "Aucune session terminée. Votre premier résultat apparaîtra ici.",
                    "Difficulté non enregistrée pour cette ancienne session",
                    "Comparez les sessions avec des réglages similaires.",
                    "Sessions",
                    "Précision",
                    "Série à 70 %",
                ],
                ["Bande de fréquence", "Amplification ou atténuation", "Gain"],
            ),
            Language::German => (
                [
                    "Sitzungsverlauf",
                    "Noch keine abgeschlossene Sitzung. Ihr erstes Ergebnis erscheint hier.",
                    "Für diese ältere Sitzung wurde die Schwierigkeit nicht gespeichert",
                    "Vergleichen Sie Sitzungen mit ähnlichen Einstellungen.",
                    "Sitzungen",
                    "Trefferquote",
                    "70-%-Serie",
                ],
                ["Frequenzband", "Anhebung oder Absenkung", "Verstärkung"],
            ),
            Language::Spanish => (
                [
                    "Historial de sesiones",
                    "Aún no hay sesiones completadas. Su primer resultado aparecerá aquí.",
                    "No se guardó la dificultad de esta sesión antigua",
                    "Compare sesiones con ajustes similares.",
                    "Sesiones",
                    "Precisión",
                    "Racha del 70 %",
                ],
                [
                    "Banda de frecuencia",
                    "Realce o atenuación",
                    "Cantidad de ganancia",
                ],
            ),
            _ => (
                [
                    "Session history",
                    "No completed sessions yet. Your first result will appear here.",
                    "Difficulty was not recorded for this older session",
                    "Compare sessions with similar settings.",
                    "Sessions",
                    "Accuracy",
                    "70% streak",
                ],
                ["Frequency band", "Boost or cut", "Gain amount"],
            ),
        };
        let localize = |text| {
            if language.is_pseudo() {
                super::pseudo_static(text)
            } else {
                text
            }
        };
        Self {
            heading: localize(labels[0]),
            empty: localize(labels[1]),
            unavailable: localize(labels[2]),
            compare: localize(labels[3]),
            exercises: exercises.map(localize),
            sessions: localize(labels[4]),
            accuracy: localize(labels[5]),
            streak: localize(labels[6]),
        }
    }
    pub fn exercise(&self, exercise: sotf_audio_player::EqTrainingExercise) -> &'static str {
        use sotf_audio_player::EqTrainingExercise;
        self.exercises[match exercise {
            EqTrainingExercise::BandIdentification => 0,
            EqTrainingExercise::BoostCutIdentification => 1,
            EqTrainingExercise::GainIdentification => 2,
        }]
    }
}
