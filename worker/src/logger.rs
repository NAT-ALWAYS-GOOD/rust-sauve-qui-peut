use std::sync::mpsc::Sender;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Local;

pub enum LogType {
    Insignificant,
    Minor,
    Medium,
    Major,
    Mandatory,
}

pub fn send_log(channel_sender: &Sender<String>, tag: &str, message: &str, log_type: LogType) {
    let now = Local::now();
    let timestamp_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let log_type_level = LogType::Major;

    if log_type as i32 >= log_type_level as i32 {
        channel_sender.send(format!("[{}] [{}] {}", timestamp_str, tag, message)).unwrap();
    }

    store_log_to_file(tag, message, now);
}

fn store_log_to_file(tag: &str, message: &str, now: chrono::DateTime<Local>) {
    let date_str = now.format("%Y%m%d").to_string();


    let tag = tag.replace(" ", "_");
    let filename = format!("../logs/{}_{}_{}.log", tag, date_str, 1);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)
        .expect("Impossible d'ouvrir ou créer le fichier de log");

    let timestamp_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    if let Err(e) = writeln!(file, "[{}] [{}] {}", timestamp_str, tag, message) {
        eprintln!("Erreur lors de l'écriture dans le fichier {}: {}", filename, e);
    }
}