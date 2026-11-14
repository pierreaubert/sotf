//! Labels for opening and saving the shared `AutoEQ` report.

// Rust guideline compliant 2026-02-21

use super::Language;

pub(crate) struct SharedReportText {
    pub open: &'static str,
    pub save: &'static str,
    pub preparing: &'static str,
    pub stale: &'static str,
}

impl SharedReportText {
    pub fn for_language(language: Language) -> Self {
        match language {
            Language::French => Self {
                open: "Ouvrir le rapport AutoEQ complet",
                save: "Enregistrer le rapport AutoEQ",
                preparing: "Préparation du rapport AutoEQ",
                stale: "Relancez RoomEQ avant d’ouvrir le rapport",
            },
            Language::German => Self {
                open: "Vollständigen AutoEQ-Bericht öffnen",
                save: "AutoEQ-Bericht speichern",
                preparing: "AutoEQ-Bericht wird vorbereitet",
                stale: "RoomEQ vor dem Öffnen des Berichts erneut ausführen",
            },
            Language::Spanish => Self {
                open: "Abrir el informe AutoEQ completo",
                save: "Guardar el informe AutoEQ",
                preparing: "Preparando el informe AutoEQ",
                stale: "Ejecute RoomEQ de nuevo antes de abrir el informe",
            },
            Language::Pseudo => Self {
                open: "[Ööpen thëë füll ÄütöEQ rëëpört]",
                save: "[Sääve thëë ÄütöEQ rëëpört]",
                preparing: "[Prëëpääriing thëë ÄütöEQ rëëpört]",
                stale: "[Rün RöömEQ äägääiin bëëföre öppëëniing thëë rëëpört]",
            },
            Language::English => Self {
                open: "Open full AutoEQ report",
                save: "Save AutoEQ report",
                preparing: "Preparing shared AutoEQ report",
                stale: "Run RoomEQ before opening the report",
            },
        }
    }
}
