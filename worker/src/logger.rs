use std::sync::mpsc::Sender;

pub enum LogType {
    Insignificant,
    Minor,
    Medium,
    Major,
    Mandatory,
}

pub fn send_log(channel_sender: &Sender<String>, tag: &str, message: &str, log_type: LogType) {
    let log_type_level = LogType::Major;

    if log_type as i32 >= log_type_level as i32 {
        channel_sender.send(format!("[{}] : {}", tag, message)).unwrap();
    }
}