/// Unique identifiers for contextual hints shown once per feature encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HintId {
    /// First time opening the Studio screen
    StudioFirstVisit,
    /// First plugin added to the rack
    FirstPluginAdded,
    /// First time on the Room EQ screen
    RoomEqFirstVisit,
    /// Empty queue shown in library
    EmptyQueue,
}

impl HintId {
    pub fn localized_title(&self, language: crate::app::i18n::Language) -> &'static str {
        use crate::app::i18n::Language;

        match (language, self) {
            (Language::Pseudo, _) => crate::app::i18n::pseudo_static(self.title()),
            (Language::English, _) => self.title(),
            (Language::French, HintId::StudioFirstVisit) => "Rack de modules",
            (Language::French, HintId::FirstPluginAdded) => "Module ajouté",
            (Language::French, HintId::RoomEqFirstVisit) => "EQ Pièce",
            (Language::French, HintId::EmptyQueue) => "Construisez votre file d’attente",
            (Language::German, HintId::StudioFirstVisit) => "Plugin-Rack",
            (Language::German, HintId::FirstPluginAdded) => "Plugin hinzugefügt",
            (Language::German, HintId::RoomEqFirstVisit) => "Raum-EQ",
            (Language::German, HintId::EmptyQueue) => "Warteschlange aufbauen",
            (Language::Spanish, HintId::StudioFirstVisit) => "Rack de plugins",
            (Language::Spanish, HintId::FirstPluginAdded) => "Plugin añadido",
            (Language::Spanish, HintId::RoomEqFirstVisit) => "EQ de sala",
            (Language::Spanish, HintId::EmptyQueue) => "Cree su cola",
        }
    }

    pub fn localized_message(&self, language: crate::app::i18n::Language) -> &'static str {
        use crate::app::i18n::Language;

        match (language, self) {
            (Language::Pseudo, _) => crate::app::i18n::pseudo_static(self.message()),
            (Language::English, _) => self.message(),
            (Language::French, HintId::StudioFirstVisit) => {
                "Raccourcis du rack : flèches pour sélectionner · Cmd/Ctrl+↑/↓ pour réordonner · Entrée pour activer · Suppr pour retirer · +/- pour ajuster · Maj+1…0 pour ajouter."
            }
            (Language::French, HintId::FirstPluginAdded) => {
                "Cliquez sur la carte du module pour modifier ses paramètres. Utilisez les touches = et - pour ajuster les valeurs."
            }
            (Language::French, HintId::RoomEqFirstVisit) => {
                "Commencez par charger les mesures, puis configurez et lancez l’optimiseur."
            }
            (Language::French, HintId::EmptyQueue) => {
                "Cliquez sur un album de la bibliothèque pour l’ajouter à la file de lecture."
            }
            (Language::German, HintId::StudioFirstVisit) => {
                "Rack-Kürzel: Pfeiltasten wählen · Cmd/Ctrl+↑/↓ sortiert um · Eingabe schaltet · Entf entfernt · +/- passt an · Umschalt+1…0 fügt hinzu."
            }
            (Language::German, HintId::FirstPluginAdded) => {
                "Klicken Sie auf die Plugin-Karte, um Parameter zu bearbeiten. Mit = und - passen Sie Werte an."
            }
            (Language::German, HintId::RoomEqFirstVisit) => {
                "Laden Sie zuerst Messdaten, konfigurieren Sie anschließend den Ablauf und starten Sie den Optimierer."
            }
            (Language::German, HintId::EmptyQueue) => {
                "Klicken Sie in der Mediathek auf ein Album, um es zur Wiedergabewarteschlange hinzuzufügen."
            }
            (Language::Spanish, HintId::StudioFirstVisit) => {
                "Atajos del rack: flechas para seleccionar · Cmd/Ctrl+↑/↓ para reordenar · Intro para activar · Supr para quitar · +/- para ajustar · Mayús+1…0 para añadir."
            }
            (Language::Spanish, HintId::FirstPluginAdded) => {
                "Haga clic en la tarjeta del plugin para editar sus parámetros. Use las teclas = y - para ajustar valores."
            }
            (Language::Spanish, HintId::RoomEqFirstVisit) => {
                "Empiece cargando las mediciones; después configure y ejecute el optimizador."
            }
            (Language::Spanish, HintId::EmptyQueue) => {
                "Haga clic en un álbum de la biblioteca para añadirlo a la cola de reproducción."
            }
        }
    }
}

impl HintId {
    pub fn as_str(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "studio_first_visit",
            HintId::FirstPluginAdded => "first_plugin_added",
            HintId::RoomEqFirstVisit => "roomeq_first_visit",
            HintId::EmptyQueue => "empty_queue",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "Plugin Rack",
            HintId::FirstPluginAdded => "Plugin Added",
            HintId::RoomEqFirstVisit => "Room EQ",
            HintId::EmptyQueue => "Build Your Queue",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => {
                "Rack shortcuts: arrow keys select · Cmd/Ctrl+↑/↓ reorder · Enter toggles · Delete removes · +/- adjusts · Shift+1…0 quick-adds."
            }
            HintId::FirstPluginAdded => {
                "Click a plugin card to edit its parameters. Use = / - keys to adjust values."
            }
            HintId::RoomEqFirstVisit => {
                "Start by loading measurement data, then configure and run the optimizer."
            }
            HintId::EmptyQueue => "Click an album in the library to add it to your playback queue.",
        }
    }
}
