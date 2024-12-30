use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use common::{ClientRequest, ServerResponse, RegisterTeamResult, SubscribePlayerResult, send_encoded_message, receive_encoded_message};


fn send_log(tx: &mpsc::Sender<String>, tag: &str, message: &str) {
    tx.send(format!("[{}] : {}", tag, message)).unwrap();
}

fn struct_to_json_bytes(client_request: &ClientRequest) -> Vec<u8> {
    serde_json::to_vec(client_request).unwrap()
}

// Enregistrement d'une équipe
fn register_team(tx: &mpsc::Sender<String>, server_address: &str) -> Option<String> {
    println!("Tentative de connexion au serveur {} pour l'équipe...", server_address);
    let team_tag = "Équipe";
    match TcpStream::connect(server_address) {
        Ok(mut stream) => {
            send_log(tx, team_tag, "Connecté au serveur pour l'équipe");

            let team_name = format!("NAT Team {}", rand::random::<u32>());
            let register_team = ClientRequest::RegisterTeam { name: team_name };

            send_log(tx, team_tag, &format!("Envoi de l'enregistrement de l'équipe : {:?}", register_team));
            let json_message = serde_json::to_vec(&register_team).unwrap();
            send_encoded_message(&mut stream, json_message);

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

fn register_player(tx: &mpsc::Sender<String>, player_name: &str, reg_token: &str, tcp_stream: &mut TcpStream) -> bool {
    let player_tag = &player_name;

    let subscribe_player = ClientRequest::SubscribePlayer {
        name: String::from(player_name),
        registration_token: String::from(reg_token),
    };

    send_log(tx, player_tag, &format!("Envoi de l'enregistrement du joueur : {:?}", subscribe_player));
    let json_message = serde_json::to_vec(&subscribe_player).unwrap();
    send_encoded_message(tcp_stream, json_message);

    send_log(tx, player_tag, "Attente de la réponse du serveur");
    match receive_encoded_message(tcp_stream) {
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Ok) => {
            send_log(tx, player_tag, "Enregistrement réussi");
            true
        }
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Err(error)) => {
            send_log(tx, player_tag, &format!("Erreur lors de l'enregistrement : {:?}", error));
            false
        }
        _ => {
            send_log(tx, player_tag, "Message inattendu reçu lors de l'enregistrement");
            false
        }
    }

}

fn main() {
    // let server_address = "[::1]:8778";
    let server_address = "localhost:8778";

    println!("Démarrage du client...");
    thread::sleep(Duration::from_secs(2));
    println!("Connexion au serveur...");
    thread::sleep(Duration::from_secs(2));

    let (log_channel_sender, log_channel_receiver) = mpsc::channel();

    // Enregistrement de l'équipe
    let reg_token = match register_team(&log_channel_sender, server_address) {
        Some(token) => token,
        None => {
            println!("Impossible d'enregistrer l'équipe.");
            println!("{}", log_channel_receiver.recv().unwrap());
            return;
        }
    };

    // Enregistrement des joueurs
    let players = vec!["Joueur 1", "Joueur 2", "Joueur 3"];
    let mut threads = vec![];

    for player in players {
        let tx_clone = log_channel_sender.clone();
        let reg_token_cloned = reg_token.clone();

        let thread = thread::spawn(move || {
            match TcpStream::connect(&server_address) {
                Ok(mut stream) => {
                    send_log(&tx_clone, player, "Connecté au serveur");
                    let registered = register_player(&tx_clone, player, &*reg_token_cloned, &mut stream);
                    if !registered {
                        // terminer le thread
                        send_log(&tx_clone, &*player, "Erreur d'enregistrement, arrêt du thread.");
                        return;
                    } else {
                        // continuer le thread
                        // recevoir le packet suivant

                    }
                },
                Err(e) => {
                    send_log(&tx_clone, &*player, &format!("Impossible de se connecter au serveur : {}", e));
                }
            }

        });

        threads.push(thread);
    }

    // Affichage des logs dans le thread principal
    for message in log_channel_receiver {
        println!("{}", message);
    }

    // Attente de la fin des threads
    for thread in threads {
        thread.join().unwrap();
    }
}