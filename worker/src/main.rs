mod maze;
mod logger;
mod decoder;
mod solver;

use crate::logger::send_log;
use crate::logger::LogType::{Insignificant, Major, Mandatory, Medium, Minor};
use crate::maze::{distance, Coord, Maze, Orientation, Player};
use crate::solver::{find_nearest_frontier, find_path_to_goal, move_to_next_step, receive_radar_view};
use common::{receive_encoded_message, send_encoded_message, Action, ClientRequest, RegisterTeamResult, ServerResponse, SubscribePlayerResult};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

fn register_team(channel_sender: &Sender<String>, server_address: &str) -> Option<String> {
    send_log(channel_sender, "Main", "Tentative de connexion au serveur pour l'équipe...", Major);
    let team_tag = "Équipe";
    match TcpStream::connect(server_address) {
        Ok(mut stream) => {
            send_log(channel_sender, team_tag, "Connecté au serveur pour l'équipe", Minor);

            let team_name = format!("NAT Team {}", rand::random::<u32>());
            let register_team = ClientRequest::RegisterTeam { name: team_name };

            send_log(channel_sender, team_tag, &format!("Envoi de l'enregistrement de l'équipe : {:?}", register_team), Insignificant);
            let json_message = serde_json::to_vec(&register_team).unwrap();
            send_encoded_message(&mut stream, json_message);

            send_log(channel_sender, team_tag, "Attente de la réponse du serveur pour l'équipe", Insignificant);
            match receive_encoded_message(&mut stream) {
                ServerResponse::RegisterTeamResult(RegisterTeamResult::Ok {
                                                       expected_players,
                                                       registration_token,
                                                   }) => {
                    send_log(
                        channel_sender,
                        team_tag,
                        &format!(
                            "Enregistrement réussi : {} joueurs attendus, token : {}",
                            expected_players, registration_token
                        ),
                        Medium,
                    );
                    Some(registration_token)
                }
                ServerResponse::RegisterTeamResult(RegisterTeamResult::Err(error)) => {
                    send_log(channel_sender, team_tag, &format!("Erreur lors de l'enregistrement de l'équipe : {:?}", error), Mandatory);
                    None
                }
                _ => {
                    send_log(channel_sender, team_tag, "Message inattendu reçu lors de l'enregistrement", Mandatory);
                    None
                }
            }
        }
        Err(e) => {
            send_log(channel_sender, team_tag, &format!("Impossible de se connecter au serveur : {}", e), Mandatory);
            None
        }
    }
}

fn register_player(channel_sender: &Sender<String>, player_name: &str, reg_token: &str, tcp_stream: &mut TcpStream) -> bool {
    let player_tag = &player_name;

    let subscribe_player = ClientRequest::SubscribePlayer {
        name: String::from(player_name),
        registration_token: String::from(reg_token),
    };

    send_log(channel_sender, player_tag, &format!("Envoi de l'enregistrement du joueur : {:?}", subscribe_player), Minor);
    let json_message = serde_json::to_vec(&subscribe_player).unwrap();
    send_encoded_message(tcp_stream, json_message);

    send_log(channel_sender, player_tag, "Attente de la réponse du serveur", Insignificant);
    match receive_encoded_message(tcp_stream) {
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Ok) => {
            send_log(channel_sender, player_tag, "Enregistrement réussi", Minor);
            true
        }
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Err(error)) => {
            send_log(channel_sender, player_tag, &format!("Erreur lors de l'enregistrement : {:?}", error), Mandatory);
            false
        }
        _ => {
            send_log(channel_sender, player_tag, "Message inattendu reçu lors de l'enregistrement", Mandatory);
            false
        }
    }
}

fn launch_player_thread(server_address: String, player: String, channel_sender_cloned: Sender<String>, reg_token_cloned: String, shared_numbers: Arc<Mutex<Vec<u64>>>, thread_index: usize) -> JoinHandle<()> {
    thread::Builder::new().name(player.clone()).spawn(move || {
        match TcpStream::connect(&server_address) {
            Ok(mut stream) => {
                send_log(&channel_sender_cloned, &*player, "Connecté au serveur", Minor);
                let registered = register_player(&channel_sender_cloned, &*player, &*reg_token_cloned, &mut stream);
                if !registered {
                    send_log(&channel_sender_cloned, &*player, "Erreur d'enregistrement, arrêt du thread.", Mandatory);
                    return;
                } else {
                    play_game(channel_sender_cloned, player, &mut stream, shared_numbers, thread_index);
                }
            }
            Err(e) => {
                send_log(&channel_sender_cloned, &*player, &format!("Impossible de se connecter au serveur : {}", e), Mandatory);
            }
        }
    }).unwrap()
}

fn play_game(channel_sender: Sender<String>, player_name: String, tcp_stream: &mut TcpStream, shared_numbers: Arc<Mutex<Vec<u64>>>, thread_index: usize) {
    let debug = false;
    let playing = true;
    let mut player = Player {
        coords: Coord { x: 0, y: 0 },
        orientation: Orientation::North,
    };
    let mut maze = Maze::new();
    let mut goal_cell: Option<Coord> = None;

    let mut instructions: Vec<Coord> = vec![]; // liste des instructions pour atteindre l'objectif

    let mut last_action: Option<Action> = None;
    let mut last_player_coords: Coord = player.coords;
    let mut last_player_orientation: Orientation = player.orientation;

    'gameloop: while playing {
        match receive_encoded_message(tcp_stream) {
            ServerResponse::RadarView(radar_view) => {
                let player_tag = format!("{}", player_name.clone());

                receive_radar_view(&channel_sender, &player_tag, debug, &mut player, &mut maze, &mut goal_cell, &radar_view);

                if let Some(goal_coord) = goal_cell {
                    if let Some(&last) = instructions.last() {
                        if last != goal_coord {
                            instructions.clear();
                        }
                    }
                    send_log(&channel_sender, &*player_tag, &format!("Objectif trouvé en {:?}, déplacement en cours...", goal_coord), Major);
                    instructions.extend(find_path_to_goal(player.coords, goal_coord, &mut maze, &channel_sender, &player_name));
                    move_to_next_step(&mut player, &player_name, instructions.remove(0), &channel_sender, tcp_stream, &mut last_action, &mut last_player_coords, &mut last_player_orientation);
                    return;
                }

                if let Some(frontier) = find_nearest_frontier(&maze, player.coords, &channel_sender, &player_name) {
                    if let Some(&last) = instructions.last() {
                        if last != frontier && distance(player.coords, frontier) != distance(player.coords, last) {
                            instructions.clear();
                        }
                    }
                    instructions.extend(find_path_to_goal(player.coords, frontier, &mut maze, &channel_sender, &player_name));
                }

                move_to_next_step(&mut player, &player_name, instructions.remove(0), &channel_sender, tcp_stream, &mut last_action, &mut last_player_coords, &mut last_player_orientation);

                send_log(&channel_sender, &*player_tag, "Dodo pendant 0,05 secondes...", Insignificant);
                thread::sleep(Duration::from_millis(50));
            }
            ServerResponse::Challenge(challenge) => {
                handle_challenge(challenge, &channel_sender, &*player_name, tcp_stream, &shared_numbers);
            }
            ServerResponse::Hint(hint) => {
                let next = handle_hint(hint, &channel_sender, &*player_name, &shared_numbers, thread_index);
                if next {
                    continue 'gameloop;
                }
            }
            ServerResponse::ActionError(action_error) => {
                let fatal = handle_action_error(action_error, &channel_sender, &*player_name, &mut last_action, &mut player, &mut last_player_coords, &mut last_player_orientation);
                if fatal {
                    break 'gameloop;
                }
            }
            msg => {
                send_log(&channel_sender, &*player_name, &format!("Message inattendu reçu : {:?}", msg), Mandatory);
            }
        }
    }
}

fn handle_hint(hint: common::Hint, channel_sender: &Sender<String>, player_name: &str, shared_numbers: &Arc<Mutex<Vec<u64>>>, thread_index: usize) -> bool {
    send_log(
        channel_sender,
        player_name,
        &format!("Indice reçu : {:?}", hint),
        Minor
    );
    match hint {
        common::Hint::Secret(number) => {
            send_log(
                channel_sender,
                player_name,
                &format!("Indice secret reçu : {}", number),
                Mandatory
            );
            shared_numbers.lock().unwrap()[thread_index] = number;
            true
        },
        common::Hint::GridSize { columns, rows } => {
            send_log(
                channel_sender,
                player_name,
                &format!("Taille de la grille : {} colonnes, {} lignes", columns, rows),
                Mandatory
            );
            true
        },
        common::Hint::RelativeCompass { angle } => {
            send_log(
                channel_sender,
                player_name,
                &format!("Indication de direction relative : angle = {}", angle),
                Mandatory
            );
            true
        }
    }
}

fn handle_action_error(action_error: common::ActionError, channel_sender: &Sender<String>, player_name: &str, last_action: &mut Option<Action>, player: &mut Player, last_player_coords: &mut Coord, last_player_orientation: &mut Orientation) -> bool {
    send_log(
        channel_sender,
        player_name,
        &format!("Erreur d'action reçue : {:?}", action_error),
        Mandatory
    );
    match action_error {
        common::ActionError::CannotPassThroughWall => {
            send_log(channel_sender, player_name, "Impossible de passer à travers un mur", Mandatory);
            player.coords = *last_player_coords;
            player.orientation = *last_player_orientation;
            send_log(
                channel_sender,
                player_name,
                &format!("Cancel de l'action précédente : {:?}", last_action),
                Medium
            );
            send_log(
                channel_sender,
                player_name,
                &format!("Nouvelles coordonnées du joueur : {:?}", last_player_coords),
                Medium
            );
        },
        common::ActionError::NoRunningChallenge => {
            send_log(channel_sender, player_name, "Aucun challenge en cours", Mandatory);
        },
        common::ActionError::SolveChallengeFirst => {
            send_log(channel_sender, player_name, "Résoudre le challenge en cours avant de se déplacer", Mandatory);
        },
        common::ActionError::InvalidChallengeSolution => {
            send_log(channel_sender, player_name, "Solution de challenge invalide", Mandatory);
            return true;
        },
    }
    false
}

fn handle_challenge(challenge: common::Challenge, channel_sender: &Sender<String>, player_name: &str, tcp_stream: &mut TcpStream, shared_numbers: &Arc<Mutex<Vec<u64>>>) {
    send_log(
        channel_sender,
        player_name,
        &format!("Challenge reçu : {:?}", challenge),
        Major
    );
    match challenge {
        common::Challenge::SecretSumModulo(number) => {
            let mut shared_mutex = shared_numbers.lock().unwrap();
            send_log(
                channel_sender,
                player_name,
                &format!("Nombres partagés : {:?}", *shared_mutex),
                Minor
            );
            let sum_mod = shared_mutex.iter().fold(0u64, |acc, &val| {
                (acc + (val % number)) % number
            });
            send_log(
                channel_sender,
                player_name,
                &format!("Somme des nombres partagés (avec modulo appliqué) : {}", sum_mod),
                Major
            );
            let action = Action::SolveChallenge { answer: sum_mod.to_string() };
            let json_message = serde_json::to_vec(&ClientRequest::Action(action.clone())).unwrap();
            send_log(
                channel_sender,
                player_name,
                &format!("Réponse au challenge : {:?}", action),
                Major
            );
            send_encoded_message(tcp_stream, json_message);
            // FIXME : remove this useless action, we can keep the last numbers we don't need to reset them
            // shared_numbers : set all 3 to 0
            *shared_mutex = vec![0; 3];
        }
        common::Challenge::SOS => {
            send_log(channel_sender, player_name, "SOS reçu, mais je ne sais pas quoi faire...", Mandatory);
        }
    }
}

fn main() {
    let (channel_sender, channel_receiver) = mpsc::channel();

    // let server_address = "[::1]:8778";
    let server_address = "localhost:8778";

    send_log(&channel_sender, "Main", "Connexion au serveur...", Major);

    let reg_token = match register_team(&channel_sender, server_address) {
        Some(token) => token,
        None => {
            send_log(&channel_sender, "Main", &format!("Impossible d'enregistrer l'équipe. {}", channel_receiver.recv().unwrap()), Mandatory);
            return;
        }
    };

    let players = vec!["Joueur 1", "Joueur 2", "Joueur 3"];
    let mut threads = vec![];

    let shared_numbers = Arc::new(Mutex::new(vec![0; 3]));

    let mut index = 0;
    for player in players {
        let thread = launch_player_thread(
            server_address.parse().unwrap(),
            player.parse().unwrap(),
            channel_sender.clone(),
            reg_token.clone(),
            Arc::clone(&shared_numbers),
            index,
        );
        index += 1;
        threads.push(thread);
    }

    // Affichage des logs dans le thread principal
    for message in channel_receiver {
        println!("{}", message);
    }

    // Attente de la fin des threads
    for thread in threads {
        thread.join().unwrap();
    }
}


