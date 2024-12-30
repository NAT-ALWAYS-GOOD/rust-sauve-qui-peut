use std::io::{Read, Write};
use std::net::TcpStream;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientRequest {
    RegisterTeam { name: String },
    SubscribePlayer { name: String, registration_token: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerResponse {
    RegisterTeamResult(RegisterTeamResult),
    SubscribePlayerResult(SubscribePlayerResult),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RegisterTeamResult {
    Ok { expected_players: u8, registration_token: String },
    Err(RegistrationError),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SubscribePlayerResult {
    Ok,
    Err(RegistrationError),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RegistrationError {
    AlreadyRegistered,
    InvalidName,
    InvalidRegistrationToken,
    TooManyPlayers,
}

pub fn send_encoded_message(stream: &mut TcpStream, json_message: Vec<u8>) {
    let mut message = Vec::new();
    message.extend((json_message.len() as u32).to_le_bytes());
    message.extend(json_message);

    if let Err(e) = stream.write_all(&message) {
        eprintln!("Erreur lors de l'envoi du message : {}", e);
    }
}

pub fn receive_encoded_message<T>(tcp_stream: &mut TcpStream) -> T where T: serde::de::DeserializeOwned {
    let mut buffer = [0u8; 4];
    tcp_stream.read_exact(&mut buffer).unwrap();
    let message_size = u32::from_le_bytes(buffer) as usize;

    let mut buffer = vec![0u8; message_size];
    tcp_stream.read_exact(&mut buffer).unwrap();

    serde_json::from_slice(&buffer).unwrap()
}