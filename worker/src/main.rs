use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};

// Définition des types et des messages
#[derive(Serialize, Deserialize, Debug)]
enum ClientRequest {
    RegisterTeam { name: String },
    SubscribePlayer { name: String, registration_token: String },
}

#[derive(Serialize, Deserialize, Debug)]
enum ServerResponse {
    RegisterTeamResult(RegisterTeamResult),
    SubscribePlayerResult(SubscribePlayerResult),
}

#[derive(Serialize, Deserialize, Debug)]
enum RegisterTeamResult {
    Ok { expected_players: u8, registration_token: String },
    Err(RegistrationError),
}

#[derive(Serialize, Deserialize, Debug)]
enum SubscribePlayerResult {
    Ok,
    Err(RegistrationError),
}

#[derive(Serialize, Deserialize, Debug)]
enum RegistrationError {
    AlreadyRegistered,
    InvalidName,
    InvalidRegistrationToken,
    TooManyPlayers,
}

// Fonction utilitaire pour envoyer des logs au canal
fn send_log(tx: &mpsc::Sender<String>, tag: &str, message: &str) {
    tx.send(format!("[{}] : {}", tag, message)).unwrap();
}

// Envoi d'un message encodé au serveur
fn send_encoded_message(stream: &mut TcpStream, client_request: &ClientRequest) {
    let json_message = serde_json::to_vec(client_request).unwrap();
    let mut message = Vec::new();
    message.extend((json_message.len() as u32).to_le_bytes());
    message.extend(json_message);

    if let Err(e) = stream.write_all(&message) {
        eprintln!("Erreur lors de l'envoi du message : {}", e);
    }
}

// Réception d'un message encodé du serveur
fn receive_encoded_message(tcp_stream: &mut TcpStream) -> ServerResponse {
    let mut buffer = [0u8; 4];
    tcp_stream.read_exact(&mut buffer).unwrap();
    let message_size = u32::from_le_bytes(buffer) as usize;

    let mut buffer = vec![0u8; message_size];
    tcp_stream.read_exact(&mut buffer).unwrap();

    serde_json::from_slice(&buffer).unwrap()
}

// Enregistrement d'une équipe
fn register_team(tx: &mpsc::Sender<String>, server_address: &str) -> Option<String> {
    let team_tag = "Équipe";
    match TcpStream::connect(server_address) {
        Ok(mut stream) => {
            send_log(tx, team_tag, "Connecté au serveur pour l'équipe");

            let team_name = format!("NAT Team {}", rand::random::<u32>());
            let register_team = ClientRequest::RegisterTeam { name: team_name };

            send_log(tx, team_tag, &format!("Envoi de l'enregistrement de l'équipe : {:?}", register_team));
            send_encoded_message(&mut stream, &register_team);

            send_log(tx, team_tag, "Attente de la réponse du serveur pour l'équipe");
            match receive_encoded_message(&mut stream) {
                ServerResponse::RegisterTeamResult(RegisterTeamResult::Ok {
                                                       expected_players,
                                                       registration_token,
                                                   }) => {
                    send_log(
                        tx,
                        team_tag,
                        &format!(
                            "Enregistrement réussi : {} joueurs attendus, token : {}",
                            expected_players, registration_token
                        ),
                    );
                    Some(registration_token)
                }
                ServerResponse::RegisterTeamResult(RegisterTeamResult::Err(error)) => {
                    send_log(tx, team_tag, &format!("Erreur lors de l'enregistrement de l'équipe : {:?}", error));
                    None
                }
                _ => {
                    send_log(tx, team_tag, "Message inattendu reçu lors de l'enregistrement");
                    None
                }
            }
        }
        Err(e) => {
            send_log(tx, team_tag, &format!("Impossible de se connecter au serveur : {}", e));
            None
        }
    }
}

// Enregistrement d'un joueur dans un thread
fn register_player(tx: mpsc::Sender<String>, player_name: String, reg_token: String, server_address: String) {
    let player_tag = &player_name;
    match TcpStream::connect(&server_address) {
        Ok(mut stream) => {
            send_log(&tx, player_tag, "Connecté au serveur");

            let subscribe_player = ClientRequest::SubscribePlayer {
                name: player_name.clone(),
                registration_token: reg_token,
            };

            send_log(&tx, player_tag, &format!("Envoi de l'enregistrement du joueur : {:?}", subscribe_player));
            send_encoded_message(&mut stream, &subscribe_player);

            send_log(&tx, player_tag, "Attente de la réponse du serveur");
            match receive_encoded_message(&mut stream) {
                ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Ok) => {
                    send_log(&tx, player_tag, "Enregistrement réussi");
                }
                ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Err(error)) => {
                    send_log(&tx, player_tag, &format!("Erreur lors de l'enregistrement : {:?}", error));
                }
                _ => {
                    send_log(&tx, player_tag, "Message inattendu reçu lors de l'enregistrement");
                }
            }
        }
        Err(e) => {
            send_log(&tx, player_tag, &format!("Impossible de se connecter au serveur : {}", e));
        }
    }
}

fn main() {
    let server_address = "[::1]:8778";

    println!("Démarrage du client...");
    thread::sleep(Duration::from_secs(2));
    println!("Connexion au serveur...");
    thread::sleep(Duration::from_secs(2));

    let (tx, rx) = mpsc::channel();

    // Enregistrement de l'équipe
    let reg_token = match register_team(&tx, server_address) {
        Some(token) => token,
        None => {
            println!("Impossible d'enregistrer l'équipe. Fin du programme.");
            return;
        }
    };

    // Enregistrement des joueurs
    let players = vec!["Joueur 1".to_string(), "Joueur 2".to_string(), "Joueur 3".to_string()];
    let mut threads = vec![];

    for player in players {
        let tx_clone = tx.clone();
        let reg_token_clone = reg_token.clone();
        let server_address_clone = server_address.to_string();

        let thread = thread::spawn(move || {
            register_player(tx_clone, player, reg_token_clone, server_address_clone);
        });

        threads.push(thread);
    }

    // Affichage des logs dans le thread principal
    for message in rx {
        println!("{}", message);
    }

    // Attente de la fin des threads
    for thread in threads {
        thread.join().unwrap();
    }
}