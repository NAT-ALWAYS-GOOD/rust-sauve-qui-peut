//! Module gérant les logs de l'application.

use crate::logger::LogType::{Minor};
use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::Sender;

/// Enumération des niveaux de log disponibles.
///
/// Les niveaux de log définissent la priorité d'un message.
/// Seuls les messages dont le niveau est supérieur ou égal à `LOG_LEVEL` seront envoyés via le canal.
pub enum LogType {
    Insignificant,
    Minor,
    Medium,
    Major,
    Mandatory,
}

/// Niveau de log minimal pour l'envoi sur le canal.
/// Seuls les logs ayant un niveau supérieur ou égal à ce seuil seront affichés sur le canal.
const LOG_LEVEL: LogType = Minor;


/// Envoie un message de log via un canal et le stocke dans un fichier.
///
/// Le message est préfixé d'un timestamp et d'un tag pour faciliter le suivi.
/// Si le niveau du log est supérieur ou égal à `LOG_LEVEL`, le message est envoyé via le canal fourni.
/// Dans tous les cas, le message est également écrit dans un fichier de log.
///
/// # Arguments
///
/// * `channel_sender` - Le canal utilisé pour envoyer les logs.
/// * `tag` - Une étiquette identifiant la source du log.
/// * `message` - Le contenu du message de log.
/// * `log_type` - Le niveau de priorité du log.
///
/// # Panique
///
/// La fonction panique si l'envoi sur le canal échoue.
pub fn send_log(channel_sender: &Sender<String>, tag: &str, message: &str, log_type: LogType) {
    let now = Local::now();
    let timestamp_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

    if log_type as i32 >= LOG_LEVEL as i32 {
        channel_sender
            .send(format!("[{}] [{}] {}", timestamp_str, tag, message))
            .unwrap();
    }

    store_log_to_file(tag, message, now);
}

/// Stocke un message de log dans un fichier.
///
/// Le fichier est créé ou ouvert dans le dossier `./logs` et porte un nom formé du tag,
/// de la date et d'un numéro de log. Le message est précédé d'un timestamp.
///
/// # Arguments
///
/// * `tag` - L'étiquette identifiant la source du log (les espaces sont remplacés par des underscores).
/// * `message` - Le message à stocker.
/// * `now` - La date et l'heure actuelles, utilisée pour le timestamp et la création du nom de fichier.
fn store_log_to_file(tag: &str, message: &str, now: chrono::DateTime<Local>) {
    let date_str = now.format("%Y%m%d").to_string();

    let tag = tag.replace(" ", "_");
    let filename = format!("./logs/{}_{}_{}.log", tag, date_str, 1);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)
        .expect("Impossible d'ouvrir ou créer le fichier de log");

    let timestamp_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    if let Err(e) = writeln!(file, "[{}] [{}] {}", timestamp_str, tag, message) {
        eprintln!(
            "Erreur lors de l'écriture dans le fichier {}: {}",
            filename, e
        );
    }
}

#[cfg(test)]
mod logger_tests {
    use super::*;

    #[test]
    fn test_major_log_when_major_expect_working() {
        let (sender, receiver) = std::sync::mpsc::channel();
        send_log(&sender, "Test", "Test log", Major);
        let received = receiver.recv().unwrap();
        assert!(received.contains("[Test] Test log"));
    }

    #[test]
    fn test_mandatory_log_when_major_expect_working() {
        let (sender, receiver) = std::sync::mpsc::channel();
        send_log(&sender, "Test", "Test log", LogType::Mandatory);
        let received = receiver.recv().unwrap();
        assert!(received.contains("[Test] Test log"));
    }

    #[test]
    fn test_minor_log_when_major_expect_not_working() {
        let (sender, receiver) = std::sync::mpsc::channel();
        send_log(&sender, "Test", "Test log", LogType::Minor);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_insignificant_log_when_major_expect_not_working() {
        let (sender, receiver) = std::sync::mpsc::channel();
        send_log(&sender, "Test", "Test log", LogType::Insignificant);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_medium_log_when_major_expect_not_working() {
        let (sender, receiver) = std::sync::mpsc::channel();
        send_log(&sender, "Test", "Test log", LogType::Medium);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_store_log_to_file_expect_working() {
        let tag = "Test";
        let message = "Test log";
        let now = Local::now();
        store_log_to_file(tag, message, now);

        let date_str = now.format("%Y%m%d").to_string();
        let tag = tag.replace(" ", "_");
        let filename = format!("../logs/{}_{}_{}.log", tag, date_str, 1);

        let content = std::fs::read_to_string(&filename).unwrap();
        assert!(content.contains("Test log"));
    }
}
