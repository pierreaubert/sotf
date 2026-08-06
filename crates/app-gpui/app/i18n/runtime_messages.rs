use super::Language;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeMessageTranslations {
    language: Language,
}

impl RuntimeMessageTranslations {
    pub fn for_language(language: Language) -> Self {
        Self { language }
    }

    pub fn translate<'a>(self, message: &'a str) -> Cow<'a, str> {
        if self.language == Language::English {
            return Cow::Borrowed(message);
        }

        for pattern in RUNTIME_MESSAGE_PATTERNS {
            let target = match self.language {
                Language::English => pattern.source,
                Language::French => pattern.french,
                Language::German => pattern.german,
                Language::Spanish => pattern.spanish,
            };
            if let Some(translated) = translate_pattern(pattern.source, target, message) {
                return Cow::Owned(translated);
            }
        }

        // Runtime errors from the engine, OS, or external plugins are the
        // explicit fallback surface and remain verbatim.
        Cow::Borrowed(message)
    }

    pub fn is_catalogued(source_template: &str) -> bool {
        RUNTIME_MESSAGE_PATTERNS
            .iter()
            .any(|pattern| pattern.source == source_template)
    }
}

#[derive(Debug, Clone, Copy)]
struct RuntimeMessagePattern {
    source: &'static str,
    french: &'static str,
    german: &'static str,
    spanish: &'static str,
}

macro_rules! message {
    ($source:literal, $french:literal, $german:literal, $spanish:literal) => {
        RuntimeMessagePattern {
            source: $source,
            french: $french,
            german: $german,
            spanish: $spanish,
        }
    };
}

const RUNTIME_MESSAGE_PATTERNS: &[RuntimeMessagePattern] = &[
    message!(
        "A SOTF server test is already running.",
        "Un test de serveur SOTF est déjà en cours.",
        "Ein SOTF-Servertest läuft bereits.",
        "Ya hay una prueba de servidor SOTF en curso."
    ),
    message!(
        "A federation scan is already running.",
        "Une analyse de fédération est déjà en cours.",
        "Ein Verbundscan läuft bereits.",
        "Ya hay un análisis de federación en curso."
    ),
    message!(
        "A remote queue command is already running.",
        "Une commande de file distante est déjà en cours.",
        "Ein entfernter Warteschlangenbefehl läuft bereits.",
        "Ya hay un comando de cola remota en curso."
    ),
    message!(
        "APO file loaded successfully",
        "Fichier APO chargé",
        "APO-Datei geladen",
        "Archivo APO cargado"
    ),
    message!(
        "Applied Headphone EQ",
        "Égalisation casque appliquée",
        "Kopfhörer-EQ angewendet",
        "EQ de auriculares aplicada"
    ),
    message!(
        "Applied {} filter Spinorama EQ",
        "{} filtre(s) Spinorama appliqué(s)",
        "{} Filter aus Spinorama-EQ angewendet",
        "{} filtro(s) de EQ Spinorama aplicado(s)"
    ),
    message!(
        "Audio command failed ({}): {}",
        "Échec de la commande audio ({}): {}",
        "Audiobefehl fehlgeschlagen ({}): {}",
        "Falló el comando de audio ({}): {}"
    ),
    message!(
        "Audio engine crashed. Please play a new track to restart.",
        "Le moteur audio s’est arrêté. Lancez une nouvelle piste pour le redémarrer.",
        "Die Audio-Engine ist abgestürzt. Starten Sie einen neuen Titel, um sie neu zu starten.",
        "El motor de audio falló. Reproduzca otra pista para reiniciarlo."
    ),
    message!(
        "Cancelling scan...",
        "Annulation de l’analyse…",
        "Scan wird abgebrochen…",
        "Cancelando el análisis…"
    ),
    message!(
        "Cannot remove subdirectory.",
        "Impossible de supprimer un sous-dossier.",
        "Unterordner kann nicht entfernt werden.",
        "No se puede quitar una subcarpeta."
    ),
    message!(
        "Cleared EQ from playback",
        "Égalisation retirée de la lecture",
        "EQ aus der Wiedergabe entfernt",
        "EQ quitada de la reproducción"
    ),
    message!(
        "Cleared local library data ({} tracks removed)",
        "Données locales effacées ({} pistes supprimées)",
        "Lokale Mediendaten gelöscht ({} Titel entfernt)",
        "Datos locales borrados ({} pistas eliminadas)"
    ),
    message!(
        "Click Copy Selected → All again to replace every channel",
        "Cliquez à nouveau sur Copier la sélection → Tous pour remplacer chaque canal",
        "Klicken Sie erneut auf Auswahl kopieren → Alle, um jeden Kanal zu ersetzen",
        "Vuelva a pulsar Copiar selección → Todos para reemplazar todos los canales"
    ),
    message!(
        "Copied {} JSON",
        "{} copié au format JSON",
        "{} als JSON kopiert",
        "{} copiado como JSON"
    ),
    message!(
        "Could not find presets directory",
        "Dossier des préréglages introuvable",
        "Preset-Ordner nicht gefunden",
        "No se encontró la carpeta de preajustes"
    ),
    message!(
        "Directory added. Press 's' to scan.",
        "Dossier ajouté. Appuyez sur « s » pour l’analyser.",
        "Ordner hinzugefügt. Drücken Sie „s“ zum Scannen.",
        "Carpeta añadida. Pulse «s» para analizar."
    ),
    message!(
        "Directory already exists.",
        "Ce dossier existe déjà.",
        "Dieser Ordner ist bereits vorhanden.",
        "La carpeta ya existe."
    ),
    message!(
        "Directory removed and cleaned up.",
        "Dossier supprimé et nettoyé.",
        "Ordner entfernt und bereinigt.",
        "Carpeta quitada y limpiada."
    ),
    message!(
        "Engine restarted, resuming playback",
        "Moteur redémarré, reprise de la lecture",
        "Engine neu gestartet, Wiedergabe wird fortgesetzt",
        "Motor reiniciado; se reanuda la reproducción"
    ),
    message!(
        "Enter a MusicBrainz search query",
        "Saisissez une recherche MusicBrainz",
        "Geben Sie eine MusicBrainz-Suchanfrage ein",
        "Introduzca una consulta de MusicBrainz"
    ),
    message!(
        "Failed to clean database: {}",
        "Échec du nettoyage de la base : {}",
        "Datenbank konnte nicht bereinigt werden: {}",
        "No se pudo limpiar la base de datos: {}"
    ),
    message!(
        "Failed to clear local library data: {}",
        "Échec de l’effacement des données locales : {}",
        "Lokale Mediendaten konnten nicht gelöscht werden: {}",
        "No se pudieron borrar los datos locales: {}"
    ),
    message!(
        "Failed to delete source: {e}",
        "Échec de la suppression de la source : {e}",
        "Quelle konnte nicht gelöscht werden: {e}",
        "No se pudo eliminar la fuente: {e}"
    ),
    message!(
        "Failed to load APO file: {}",
        "Échec du chargement APO : {}",
        "APO-Datei konnte nicht geladen werden: {}",
        "No se pudo cargar el archivo APO: {}"
    ),
    message!(
        "Failed to load SOFA file: {}",
        "Échec du chargement SOFA : {}",
        "SOFA-Datei konnte nicht geladen werden: {}",
        "No se pudo cargar el archivo SOFA: {}"
    ),
    message!(
        "Failed to save source: {e}",
        "Échec de la sauvegarde de la source : {e}",
        "Quelle konnte nicht gespeichert werden: {e}",
        "No se pudo guardar la fuente: {e}"
    ),
    message!(
        "Failed to save: {}",
        "Échec de la sauvegarde : {}",
        "Speichern fehlgeschlagen: {}",
        "No se pudo guardar: {}"
    ),
    message!(
        "Failed to start HAL: {}",
        "Échec du démarrage HAL : {}",
        "HAL konnte nicht gestartet werden: {}",
        "No se pudo iniciar HAL: {}"
    ),
    message!(
        "Failed to start ReplayGain scan: {}",
        "Échec du lancement de l’analyse ReplayGain : {}",
        "ReplayGain-Scan konnte nicht gestartet werden: {}",
        "No se pudo iniciar el análisis ReplayGain: {}"
    ),
    message!(
        "Failed to start bliss analysis scan: {}",
        "Échec du lancement de l’analyse Bliss : {}",
        "Bliss-Analyse konnte nicht gestartet werden: {}",
        "No se pudo iniciar el análisis Bliss: {}"
    ),
    message!(
        "Failed to start waveform analysis: {}",
        "Échec du lancement de l’analyse de forme d’onde : {}",
        "Wellenformanalyse konnte nicht gestartet werden: {}",
        "No se pudo iniciar el análisis de forma de onda: {}"
    ),
    message!(
        "Failed to {action}: {err}",
        "Échec de l’action {action} : {err}",
        "Aktion {action} fehlgeschlagen: {err}",
        "Falló la acción {action}: {err}"
    ),
    message!(
        "Failed to {action}; restored previous source settings: {e}",
        "Échec de l’action {action} ; paramètres précédents restaurés : {e}",
        "Aktion {action} fehlgeschlagen; vorherige Quelleinstellungen wiederhergestellt: {e}",
        "Falló la acción {action}; se restauró la configuración anterior: {e}"
    ),
    message!(
        "Failed to {action}; source availability was not saved: {e}",
        "Échec de l’action {action} ; disponibilité non sauvegardée : {e}",
        "Aktion {action} fehlgeschlagen; Verfügbarkeit wurde nicht gespeichert: {e}",
        "Falló la acción {action}; no se guardó la disponibilidad: {e}"
    ),
    message!(
        "Format error: {e}",
        "Erreur de format : {e}",
        "Formatfehler: {e}",
        "Error de formato: {e}"
    ),
    message!(
        "Found {merged} SOTF server(s).",
        "{merged} serveur(s) SOTF trouvé(s).",
        "{merged} SOTF-Server gefunden.",
        "{merged} servidor(es) SOTF encontrado(s)."
    ),
    message!(
        "Found {} external plugins",
        "{} modules externes trouvés",
        "{} externe Plugins gefunden",
        "{} complementos externos encontrados"
    ),
    message!(
        "Imported community theme",
        "Thème communautaire importé",
        "Community-Theme importiert",
        "Tema comunitario importado"
    ),
    message!(
        "Imported files added, but scan could not start: {err}",
        "Fichiers ajoutés, mais l’analyse n’a pas démarré : {err}",
        "Dateien hinzugefügt, Scan konnte jedoch nicht starten: {err}",
        "Se añadieron los archivos, pero no pudo iniciarse el análisis: {err}"
    ),
    message!(
        "Imported files queued {added} library location(s) for scanning.",
        "{added} emplacement(s) importé(s) mis en attente d’analyse.",
        "{added} importierte Medienordner zum Scannen vorgemerkt.",
        "{added} ubicación(es) importada(s) en cola para analizar."
    ),
    message!(
        "Loaded preset: {}",
        "Préréglage chargé : {}",
        "Preset geladen: {}",
        "Preajuste cargado: {}"
    ),
    message!(
        "Loaded preset: {} ({} plugin(s) skipped: {})",
        "Préréglage chargé : {} ({} module(s) ignoré(s) : {})",
        "Preset geladen: {} ({} Plugin(s) übersprungen: {})",
        "Preajuste cargado: {} ({} complemento(s) omitido(s): {})"
    ),
    message!(
        "Loaded preset: {} ({} plugins)",
        "Préréglage chargé : {} ({} modules)",
        "Preset geladen: {} ({} Plugins)",
        "Preajuste cargado: {} ({} complementos)"
    ),
    message!(
        "Loaded preset: {} ({} plugins, {} skipped: {})",
        "Préréglage chargé : {} ({} modules, {} ignorés : {})",
        "Preset geladen: {} ({} Plugins, {} übersprungen: {})",
        "Preajuste cargado: {} ({} complementos, {} omitidos: {})"
    ),
    message!(
        "Low Power Mode disabled; restored motion setting.",
        "Mode économie désactivé ; animation restaurée.",
        "Stromsparmodus deaktiviert; Bewegungseinstellung wiederhergestellt.",
        "Modo de bajo consumo desactivado; movimiento restaurado."
    ),
    message!(
        "Low Power Mode enabled; reduced motion is active.",
        "Mode économie activé ; animations réduites.",
        "Stromsparmodus aktiviert; reduzierte Bewegung ist aktiv.",
        "Modo de bajo consumo activado; movimiento reducido."
    ),
    message!(
        "Metadata updated for {} file(s)",
        "Métadonnées mises à jour pour {} fichier(s)",
        "Metadaten für {} Datei(en) aktualisiert",
        "Metadatos actualizados en {} archivo(s)"
    ),
    message!(
        "MusicBrainz is disabled in Metadata settings",
        "MusicBrainz est désactivé dans les paramètres de métadonnées",
        "MusicBrainz ist in den Metadaten-Einstellungen deaktiviert",
        "MusicBrainz está desactivado en los ajustes de metadatos"
    ),
    message!(
        "No EQ result to apply",
        "Aucun résultat EQ à appliquer",
        "Kein EQ-Ergebnis zum Anwenden",
        "No hay resultado EQ que aplicar"
    ),
    message!(
        "No EQ result to save",
        "Aucun résultat EQ à sauvegarder",
        "Kein EQ-Ergebnis zum Speichern",
        "No hay resultado EQ que guardar"
    ),
    message!(
        "No SOTF servers found on the local network.",
        "Aucun serveur SOTF trouvé sur le réseau local.",
        "Keine SOTF-Server im lokalen Netzwerk gefunden.",
        "No se encontraron servidores SOTF en la red local."
    ),
    message!(
        "No filename specified",
        "Aucun nom de fichier indiqué",
        "Kein Dateiname angegeben",
        "No se indicó un nombre de archivo"
    ),
    message!(
        "No filters in EQ result",
        "Aucun filtre dans le résultat EQ",
        "Keine Filter im EQ-Ergebnis",
        "No hay filtros en el resultado EQ"
    ),
    message!(
        "No missing tracks found in database",
        "Aucune piste absente trouvée dans la base",
        "Keine fehlenden Titel in der Datenbank gefunden",
        "No se encontraron pistas ausentes en la base"
    ),
    message!(
        "No optimization result to apply",
        "Aucun résultat d’optimisation à appliquer",
        "Kein Optimierungsergebnis zum Anwenden",
        "No hay resultado de optimización que aplicar"
    ),
    message!(
        "No optimization result to save",
        "Aucun résultat d’optimisation à sauvegarder",
        "Kein Optimierungsergebnis zum Speichern",
        "No hay resultado de optimización que guardar"
    ),
    message!(
        "No optimization results to apply. Run the optimizer first.",
        "Aucun résultat à appliquer. Lancez d’abord l’optimiseur.",
        "Keine Optimierungsergebnisse. Starten Sie zuerst den Optimierer.",
        "No hay resultados que aplicar. Ejecute primero el optimizador."
    ),
    message!(
        "Optimization failed: {}",
        "Échec de l’optimisation : {}",
        "Optimierung fehlgeschlagen: {}",
        "Falló la optimización: {}"
    ),
    message!(
        "Overwritten preset: {}",
        "Préréglage remplacé : {}",
        "Preset überschrieben: {}",
        "Preajuste sobrescrito: {}"
    ),
    message!(
        "Playback error: {}",
        "Erreur de lecture : {}",
        "Wiedergabefehler: {}",
        "Error de reproducción: {}"
    ),
    message!(
        "Playing {} on remote server.",
        "Lecture de {} sur le serveur distant.",
        "{} wird auf dem entfernten Server abgespielt.",
        "Reproduciendo {} en el servidor remoto."
    ),
    message!(
        "Please select a custom target curve file",
        "Sélectionnez un fichier de courbe cible personnalisée",
        "Wählen Sie eine benutzerdefinierte Zielkurvendatei",
        "Seleccione un archivo de curva objetivo personalizada"
    ),
    message!(
        "Please select a measurement file",
        "Sélectionnez un fichier de mesure",
        "Wählen Sie eine Messdatei",
        "Seleccione un archivo de medición"
    ),
    message!(
        "Plugin update failed: {}",
        "Échec de la mise à jour du module : {}",
        "Plugin-Aktualisierung fehlgeschlagen: {}",
        "Falló la actualización del complemento: {}"
    ),
    message!(
        "Preset '{}': {} plugin(s) skipped",
        "Préréglage « {} » : {} module(s) ignoré(s)",
        "Preset „{}“: {} Plugin(s) übersprungen",
        "Preajuste «{}»: {} complemento(s) omitido(s)"
    ),
    message!(
        "Rack backup saved",
        "Sauvegarde du rack enregistrée",
        "Rack-Sicherung gespeichert",
        "Copia de seguridad del rack guardada"
    ),
    message!(
        "Remote album ID is missing.",
        "Identifiant d’album distant absent.",
        "Entfernte Album-ID fehlt.",
        "Falta el identificador del álbum remoto."
    ),
    message!(
        "Remote events: {err}",
        "Événements distants : {err}",
        "Entfernte Ereignisse: {err}",
        "Eventos remotos: {err}"
    ),
    message!(
        "Remote queue command worker disconnected.",
        "Le worker de file distante s’est déconnecté.",
        "Worker für entfernte Warteschlange getrennt.",
        "El proceso de cola remota se desconectó."
    ),
    message!(
        "Remote queue: {err}",
        "File distante : {err}",
        "Entfernte Warteschlange: {err}",
        "Cola remota: {err}"
    ),
    message!(
        "Remote server: {message}",
        "Serveur distant : {message}",
        "Entfernter Server: {message}",
        "Servidor remoto: {message}"
    ),
    message!(
        "Removed {} missing tracks from database",
        "{} piste(s) absente(s) supprimée(s) de la base",
        "{} fehlende Titel aus der Datenbank entfernt",
        "{} pista(s) ausente(s) eliminada(s) de la base"
    ),
    message!(
        "Room EQ graph applied successfully",
        "Graphe de correction de salle appliqué",
        "Raum-EQ-Graph erfolgreich angewendet",
        "Grafo de EQ de sala aplicado"
    ),
    message!(
        "SOFA file loaded successfully",
        "Fichier SOFA chargé",
        "SOFA-Datei geladen",
        "Archivo SOFA cargado"
    ),
    message!(
        "SOTF API token required for this server.",
        "Un jeton API SOTF est requis pour ce serveur.",
        "Für diesen Server ist ein SOTF-API-Token erforderlich.",
        "Este servidor requiere un token de API SOTF."
    ),
    message!(
        "SOTF API token required. Enter the token from the server settings.",
        "Jeton API SOTF requis. Saisissez celui des paramètres du serveur.",
        "SOTF-API-Token erforderlich. Geben Sie das Token aus den Servereinstellungen ein.",
        "Se requiere un token de API SOTF. Introdúzcalo desde los ajustes del servidor."
    ),
    message!(
        "SOTF discovery failed: {err}",
        "Échec de la découverte SOTF : {err}",
        "SOTF-Erkennung fehlgeschlagen: {err}",
        "Falló la detección SOTF: {err}"
    ),
    message!(
        "SOTF server added from QR code, but the API token could not be stored.",
        "Serveur SOTF ajouté par QR, mais le jeton n’a pas pu être stocké.",
        "SOTF-Server per QR hinzugefügt, API-Token konnte jedoch nicht gespeichert werden.",
        "Servidor SOTF añadido por QR, pero no se pudo guardar el token."
    ),
    message!(
        "SOTF server added from QR code.",
        "Serveur SOTF ajouté par code QR.",
        "SOTF-Server per QR-Code hinzugefügt.",
        "Servidor SOTF añadido por código QR."
    ),
    message!(
        "SOTF server saved, but the API token could not be stored in Keychain.",
        "Serveur SOTF sauvegardé, mais le jeton n’a pas pu être stocké dans le Trousseau.",
        "SOTF-Server gespeichert, API-Token konnte jedoch nicht im Schlüsselbund gespeichert werden.",
        "Servidor SOTF guardado, pero no se pudo guardar el token en el llavero."
    ),
    message!(
        "SOTF server saved.",
        "Serveur SOTF sauvegardé.",
        "SOTF-Server gespeichert.",
        "Servidor SOTF guardado."
    ),
    message!(
        "SOTF server test failed: {err}",
        "Échec du test du serveur SOTF : {err}",
        "SOTF-Servertest fehlgeschlagen: {err}",
        "Falló la prueba del servidor SOTF: {err}"
    ),
    message!(
        "Saved EQ to {}",
        "Égalisation sauvegardée dans {}",
        "EQ gespeichert unter {}",
        "EQ guardada en {}"
    ),
    message!(
        "Saved preset: {}",
        "Préréglage sauvegardé : {}",
        "Preset gespeichert: {}",
        "Preajuste guardado: {}"
    ),
    message!(
        "Saved to {}",
        "Sauvegardé dans {}",
        "Gespeichert unter {}",
        "Guardado en {}"
    ),
    message!(
        "Scan complete but failed to reload library.",
        "Analyse terminée, mais rechargement de la bibliothèque impossible.",
        "Scan abgeschlossen, Mediathek konnte jedoch nicht neu geladen werden.",
        "Análisis completado, pero no se pudo recargar la biblioteca."
    ),
    message!(
        "Scan complete. Library now has {} tracks in {} albums.",
        "Analyse terminée. La bibliothèque contient {} pistes dans {} albums.",
        "Scan abgeschlossen. Die Mediathek enthält {} Titel in {} Alben.",
        "Análisis completado. La biblioteca contiene {} pistas en {} álbumes."
    ),
    message!(
        "Scan complete: {} albums, {} tracks merged.",
        "Analyse terminée : {} albums et {} pistes fusionnés.",
        "Scan abgeschlossen: {} Alben, {} Titel zusammengeführt.",
        "Análisis completado: {} álbumes y {} pistas combinados."
    ),
    message!(
        "Scan completed, but failed to save source sync time: {e}",
        "Analyse terminée, mais l’heure de synchronisation n’a pas été sauvegardée : {e}",
        "Scan abgeschlossen, Synchronisationszeit konnte jedoch nicht gespeichert werden: {e}",
        "Análisis completado, pero no se guardó la hora de sincronización: {e}"
    ),
    message!(
        "Scan failed: {err}",
        "Échec de l’analyse : {err}",
        "Scan fehlgeschlagen: {err}",
        "Falló el análisis: {err}"
    ),
    message!(
        "Scan failed: {}",
        "Échec de l’analyse : {}",
        "Scan fehlgeschlagen: {}",
        "Falló el análisis: {}"
    ),
    message!(
        "Select a SOTF remote player first.",
        "Sélectionnez d’abord un lecteur SOTF distant.",
        "Wählen Sie zuerst einen entfernten SOTF-Player.",
        "Seleccione primero un reproductor SOTF remoto."
    ),
    message!(
        "Source added.",
        "Source ajoutée.",
        "Quelle hinzugefügt.",
        "Fuente añadida."
    ),
    message!(
        "Source removed.",
        "Source supprimée.",
        "Quelle entfernt.",
        "Fuente eliminada."
    ),
    message!(
        "Updated text size from iOS Dynamic Type.",
        "Taille du texte mise à jour depuis Dynamic Type iOS.",
        "Textgröße aus iOS Dynamic Type aktualisiert.",
        "Tamaño de texto actualizado desde Dynamic Type de iOS."
    ),
    message!(
        "iOS memory warning received; released cached library artwork.",
        "Alerte mémoire iOS reçue ; illustrations en cache libérées.",
        "iOS-Speicherwarnung; zwischengespeicherte Cover freigegeben.",
        "Aviso de memoria de iOS; se liberaron las carátulas en caché."
    ),
    message!(
        "{:?} is not available on the {:?} release channel",
        "{:?} n’est pas disponible sur le canal {:?}",
        "{:?} ist im Release-Kanal {:?} nicht verfügbar",
        "{:?} no está disponible en el canal {:?}"
    ),
    message!(
        "{friendly_name} is reachable.",
        "{friendly_name} est accessible.",
        "{friendly_name} ist erreichbar.",
        "{friendly_name} está disponible."
    ),
    message!(
        "All channels recorded, saving...",
        "Tous les canaux sont enregistrés, sauvegarde…",
        "Alle Kanäle aufgenommen, Speichern…",
        "Todos los canales grabados; guardando…"
    ),
    message!(
        "Backup saved to {}",
        "Sauvegarde enregistrée dans {}",
        "Sicherung gespeichert unter {}",
        "Copia guardada en {}"
    ),
    message!(
        "Cancelling — finishing current iteration...",
        "Annulation — fin de l’itération en cours…",
        "Abbruch – aktuelle Iteration wird beendet…",
        "Cancelando; se termina la iteración actual…"
    ),
    message!(
        "Cannot create recording directory: {}",
        "Impossible de créer le dossier d’enregistrement : {}",
        "Aufnahmeordner kann nicht erstellt werden: {}",
        "No se puede crear la carpeta de grabación: {}"
    ),
    message!("Complete!", "Terminé !", "Fertig!", "¡Completado!"),
    message!("Error: {}", "Erreur : {}", "Fehler: {}", "Error: {}"),
    message!(
        "Exported {} to {}",
        "{} exporté dans {}",
        "{} nach {} exportiert",
        "{} exportado a {}"
    ),
    message!(
        "Failed to prepare output directory: {}",
        "Échec de la préparation du dossier de sortie : {}",
        "Ausgabeordner konnte nicht vorbereitet werden: {}",
        "No se pudo preparar la carpeta de salida: {}"
    ),
    message!(
        "Failed to read: {}",
        "Échec de la lecture : {}",
        "Lesen fehlgeschlagen: {}",
        "No se pudo leer: {}"
    ),
    message!(
        "Failed to save: {}",
        "Échec de la sauvegarde : {}",
        "Speichern fehlgeschlagen: {}",
        "No se pudo guardar: {}"
    ),
    message!(
        "Failed to serialize: {}",
        "Échec de la sérialisation : {}",
        "Serialisierung fehlgeschlagen: {}",
        "No se pudo serializar: {}"
    ),
    message!(
        "Legacy file migration is no longer supported. Re-record to regenerate the file.",
        "La migration des anciens fichiers n’est plus prise en charge. Réenregistrez ou régénérez ce fichier.",
        "Die Migration alter Dateien wird nicht mehr unterstützt. Nehmen Sie neu auf oder erzeugen Sie die Datei erneut.",
        "Ya no se admite la migración de archivos antiguos. Vuelva a grabar o genere el archivo de nuevo."
    ),
    message!(
        "Loaded {} channel(s) (truncated from {} — upgrade release channel for more)",
        "{} canal(aux) chargé(s) (limité à {} — changez de canal de publication pour plus)",
        "{} Kanal/Kanäle geladen (auf {} begrenzt – Release-Kanal für mehr ändern)",
        "{} canal(es) cargado(s) (limitado a {}; cambie el canal de versión para más)"
    ),
    message!(
        "Loaded {} channel(s) from {} (truncated from {} — upgrade release channel for more)",
        "{} canal(aux) chargé(s) depuis {} (limité à {} — changez de canal de publication pour plus)",
        "{} Kanal/Kanäle aus {} geladen (auf {} begrenzt – Release-Kanal für mehr ändern)",
        "{} canal(es) cargado(s) desde {} (limitado a {}; cambie el canal de versión para más)"
    ),
    message!(
        "Loaded {} channels from {}",
        "{} canaux chargés depuis {}",
        "{} Kanäle aus {} geladen",
        "{} canales cargados desde {}"
    ),
    message!(
        "Loading measurement data...",
        "Chargement des mesures…",
        "Messdaten werden geladen…",
        "Cargando datos de medición…"
    ),
    message!(
        "Move microphones to position {} and click Continue",
        "Placez les microphones en position {} puis cliquez sur Continuer",
        "Mikrofone an Position {} stellen und auf Weiter klicken",
        "Mueva los micrófonos a la posición {} y pulse Continuar"
    ),
    message!(
        "Optimization cancelled",
        "Optimisation annulée",
        "Optimierung abgebrochen",
        "Optimización cancelada"
    ),
    message!(
        "Optimization complete! Score: {:.2} -> {:.2}",
        "Optimisation terminée ! Score : {:.2} → {:.2}",
        "Optimierung abgeschlossen! Wert: {:.2} → {:.2}",
        "¡Optimización completada! Puntuación: {:.2} → {:.2}"
    ),
    message!(
        "Optimization completed successfully",
        "Optimisation terminée",
        "Optimierung erfolgreich abgeschlossen",
        "Optimización completada"
    ),
    message!(
        "Optimizing all channels (parallel)...",
        "Optimisation de tous les canaux en parallèle…",
        "Alle Kanäle werden parallel optimiert…",
        "Optimizando todos los canales en paralelo…"
    ),
    message!(
        "Optimizing{}",
        "Optimisation{}",
        "Optimierung{}",
        "Optimizando{}"
    ),
    message!(
        "Please select a recording directory in the Configuration step",
        "Sélectionnez un dossier d’enregistrement à l’étape Configuration",
        "Wählen Sie im Schritt Konfiguration einen Aufnahmeordner",
        "Seleccione una carpeta de grabación en el paso Configuración"
    ),
    message!(
        "QA RoomEQ JSON exported: {}",
        "JSON QA RoomEQ exporté : {}",
        "RoomEQ-QA-JSON exportiert: {}",
        "JSON de QA RoomEQ exportado: {}"
    ),
    message!(
        "QA RoomEQ fixture loaded with default wizard preset",
        "Fixture QA RoomEQ chargée avec le préréglage par défaut",
        "RoomEQ-QA-Fixture mit Standardassistent geladen",
        "Fixture de QA RoomEQ cargado con el preajuste predeterminado"
    ),
    message!(
        "QA RoomEQ fixture loaded: {} channels",
        "Fixture QA RoomEQ chargée : {} canaux",
        "RoomEQ-QA-Fixture geladen: {} Kanäle",
        "Fixture de QA RoomEQ cargado: {} canales"
    ),
    message!(
        "Recording error: {}",
        "Erreur d’enregistrement : {}",
        "Aufnahmefehler: {}",
        "Error de grabación: {}"
    ),
    message!(
        "Recording position {}...",
        "Enregistrement de la position {}…",
        "Position {} wird aufgenommen…",
        "Grabando la posición {}…"
    ),
    message!(
        "Recording session cancelled",
        "Session d’enregistrement annulée",
        "Aufnahmesitzung abgebrochen",
        "Sesión de grabación cancelada"
    ),
    message!(
        "Recording stopped",
        "Enregistrement arrêté",
        "Aufnahme gestoppt",
        "Grabación detenida"
    ),
    message!(
        "Recording {}...",
        "Enregistrement de {}…",
        "{} wird aufgenommen…",
        "Grabando {}…"
    ),
    message!(
        "Room EQ applied as graph!",
        "Correction de salle appliquée comme graphe !",
        "Raum-EQ als Graph angewendet!",
        "¡EQ de sala aplicada como grafo!"
    ),
    message!(
        "Saved to {}{}",
        "Sauvegardé dans {}{}",
        "Gespeichert unter {}{}",
        "Guardado en {}{}"
    ),
    message!(
        "Starting optimization...",
        "Démarrage de l’optimisation…",
        "Optimierung wird gestartet…",
        "Iniciando optimización…"
    ),
    message!(
        "Successfully loaded {} channel(s) from recording session",
        "{} canal(aux) chargé(s) depuis la session d’enregistrement",
        "{} Kanal/Kanäle aus der Aufnahmesitzung geladen",
        "{} canal(es) cargado(s) desde la sesión de grabación"
    ),
    message!(
        "Successfully loaded {} channel(s) from {} (RoomConfig format)",
        "{} canal(aux) chargé(s) depuis {} (format RoomConfig)",
        "{} Kanal/Kanäle aus {} geladen (RoomConfig-Format)",
        "{} canal(es) cargado(s) desde {} (formato RoomConfig)"
    ),
    message!(
        "{} is not in the current RoomConfig format — re-run the Recording wizard to regenerate it.",
        "{} n’est pas au format RoomConfig actuel — relancez l’assistant Enregistrement.",
        "{} hat nicht das aktuelle RoomConfig-Format – führen Sie den Aufnahmeassistenten erneut aus.",
        "{} no usa el formato RoomConfig actual; vuelva a ejecutar el asistente de grabación."
    ),
    message!(
        "{} recording complete",
        "Enregistrement de {} terminé",
        "Aufnahme von {} abgeschlossen",
        "Grabación de {} completada"
    ),
    message!(
        "Export failed: {}",
        "Échec de l’export : {}",
        "Export fehlgeschlagen: {}",
        "Falló la exportación: {}"
    ),
    message!(
        "Failed to apply graph: {}",
        "Échec de l’application du graphe : {}",
        "Graph konnte nicht angewendet werden: {}",
        "No se pudo aplicar el grafo: {}"
    ),
    message!(
        "Failed to apply room EQ: {}",
        "Échec de l’application de la correction de salle : {}",
        "Raum-EQ konnte nicht angewendet werden: {}",
        "No se pudo aplicar la EQ de sala: {}"
    ),
    message!(
        "Failed to apply: {}",
        "Échec de l’application : {}",
        "Anwenden fehlgeschlagen: {}",
        "No se pudo aplicar: {}"
    ),
    message!(
        "Failed to load measurements: no valid channel data found.",
        "Échec du chargement : aucune donnée de canal valide.",
        "Messungen konnten nicht geladen werden: keine gültigen Kanaldaten.",
        "No se pudieron cargar las mediciones: no hay datos de canal válidos."
    ),
    message!(
        "Failed to package WAV files: {}",
        "Échec de la création du paquet WAV : {}",
        "WAV-Dateien konnten nicht gepackt werden: {}",
        "No se pudieron empaquetar los archivos WAV: {}"
    ),
    message!(
        "Failed to parse measurements: {}",
        "Échec de l’analyse des mesures : {}",
        "Messungen konnten nicht gelesen werden: {}",
        "No se pudieron interpretar las mediciones: {}"
    ),
    message!(
        "Failed to read file: {}",
        "Échec de la lecture du fichier : {}",
        "Datei konnte nicht gelesen werden: {}",
        "No se pudo leer el archivo: {}"
    ),
    message!(
        "Failed to save backup: {}",
        "Échec de la sauvegarde de secours : {}",
        "Sicherung konnte nicht gespeichert werden: {}",
        "No se pudo guardar la copia: {}"
    ),
    message!(
        "Failed to serialize: {}",
        "Échec de la sérialisation : {}",
        "Serialisierung fehlgeschlagen: {}",
        "No se pudo serializar: {}"
    ),
    message!(
        "Failed to write: {}",
        "Échec de l’écriture : {}",
        "Schreiben fehlgeschlagen: {}",
        "No se pudo escribir: {}"
    ),
    message!(
        "Headphone download state unavailable",
        "État du téléchargement casque indisponible",
        "Kopfhörer-Downloadstatus nicht verfügbar",
        "Estado de descarga de auriculares no disponible"
    ),
    message!(
        "Headphone fetch state unavailable",
        "État de recherche casque indisponible",
        "Kopfhörer-Abrufstatus nicht verfügbar",
        "Estado de búsqueda de auriculares no disponible"
    ),
    message!(
        "Internal error: {}",
        "Erreur interne : {}",
        "Interner Fehler: {}",
        "Error interno: {}"
    ),
    message!(
        "No channels to optimize",
        "Aucun canal à optimiser",
        "Keine Kanäle zum Optimieren",
        "No hay canales que optimizar"
    ),
    message!(
        "No completed recordings found. Please record measurements first.",
        "Aucun enregistrement terminé. Enregistrez d’abord des mesures.",
        "Keine abgeschlossenen Aufnahmen gefunden. Nehmen Sie zuerst Messungen auf.",
        "No hay grabaciones completadas. Grabe primero las mediciones."
    ),
    message!(
        "No optimization results to apply",
        "Aucun résultat d’optimisation à appliquer",
        "Keine Optimierungsergebnisse zum Anwenden",
        "No hay resultados de optimización que aplicar"
    ),
    message!(
        "No optimization results to export",
        "Aucun résultat d’optimisation à exporter",
        "Keine Optimierungsergebnisse zum Exportieren",
        "No hay resultados de optimización que exportar"
    ),
    message!(
        "No speaker selected",
        "Aucune enceinte sélectionnée",
        "Kein Lautsprecher ausgewählt",
        "No se seleccionó ningún altavoz"
    ),
    message!(
        "No valid measurement data found. The file may be an older format — try re-recording or re-exporting.",
        "Aucune mesure valide. Le fichier est peut-être ancien — réenregistrez ou réexportez-le.",
        "Keine gültigen Messdaten. Die Datei kann ein altes Format haben – nehmen oder exportieren Sie erneut.",
        "No hay mediciones válidas. El archivo puede ser antiguo; vuelva a grabar o exportar."
    ),
    message!(
        "Room optimization error: {}",
        "Erreur d’optimisation de salle : {}",
        "Fehler bei der Raumoptimierung: {}",
        "Error de optimización de sala: {}"
    ),
];

fn translate_pattern(source: &str, target: &str, message: &str) -> Option<String> {
    let source_segments = template_segments(source);
    if source_segments.len() == 1 {
        return (message == source).then(|| target.to_string());
    }

    let first = source_segments.first()?;
    if !message.starts_with(first) {
        return None;
    }

    let mut captures = Vec::with_capacity(source_segments.len() - 1);
    let mut cursor = first.len();
    for (index, segment) in source_segments.iter().enumerate().skip(1) {
        if segment.is_empty() && index == source_segments.len() - 1 {
            captures.push(&message[cursor..]);
            cursor = message.len();
            continue;
        }
        let relative = message[cursor..].find(segment)?;
        let end = cursor + relative;
        captures.push(&message[cursor..end]);
        cursor = end + segment.len();
    }
    if cursor != message.len() {
        return None;
    }

    let target_segments = template_segments(target);
    if target_segments.len() != captures.len() + 1 {
        return None;
    }
    let mut translated = String::with_capacity(message.len() + 16);
    for (index, segment) in target_segments.iter().enumerate() {
        translated.push_str(segment);
        if let Some(value) = captures.get(index) {
            translated.push_str(value);
        }
    }
    Some(translated)
}

fn template_segments(template: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    while let Some(open_relative) = template[cursor..].find('{') {
        let open = cursor + open_relative;
        let Some(close_relative) = template[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_relative;
        segments.push(&template[cursor..open]);
        cursor = close + 1;
    }
    segments.push(&template[cursor..]);
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatted_messages_preserve_dynamic_values() {
        let text = RuntimeMessageTranslations::for_language(Language::French);
        assert_eq!(
            text.translate("Scan complete: 12 albums, 345 tracks merged."),
            "Analyse terminée : 12 albums et 345 pistes fusionnés."
        );
        assert_eq!(
            text.translate("Playback error: device vanished"),
            "Erreur de lecture : device vanished"
        );
    }

    #[test]
    fn external_errors_are_the_explicit_fallback() {
        let text = RuntimeMessageTranslations::for_language(Language::German);
        assert_eq!(
            text.translate("External plugin vendor error"),
            "External plugin vendor error"
        );
    }

    #[test]
    fn every_catalog_entry_has_matching_placeholders() {
        for pattern in RUNTIME_MESSAGE_PATTERNS {
            let count = template_segments(pattern.source).len();
            assert_eq!(template_segments(pattern.french).len(), count);
            assert_eq!(template_segments(pattern.german).len(), count);
            assert_eq!(template_segments(pattern.spanish).len(), count);
        }
    }
}
