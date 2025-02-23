use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use common::{receive_encoded_message, send_encoded_message, ClientRequest, RegisterTeamResult, RegistrationError, ServerResponse, SubscribePlayerResult, Action, RelativeDirection};

mod encoder;

fn get_steps() -> Vec<Vec<Vec<u8>>> {
    vec![
        vec![
            vec![2, 0, 2, 1, 2, 0, 2],
            vec![0, 0, 2, 1, 2, 0, 0],
            vec![2, 2, 2, 1, 2, 0, 2],
            vec![2, 1, 1, 1, 2, 0, 0],
            vec![2, 1, 2, 2, 2, 0, 2],
            vec![2, 10, 0, 0, 0, 0, 0],
            vec![2, 2, 2, 0, 2, 0, 2],
        ],
        vec![
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![2, 2, 2, 2, 2, 0, 2],
            vec![2, 10, 1, 1, 2, 0, 0],
            vec![2, 1, 2, 1, 2, 2, 2],
            vec![0, 0, 2, 1, 1, 1, 1],
            vec![2, 0, 2, 2, 2, 2, 2],
        ],
        vec![
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![2, 2, 2, 2, 2, 0, 0],
            vec![2, 1, 1, 10, 2, 0, 0],
            vec![2, 2, 2, 1, 2, 0, 0],
            vec![2, 1, 1, 1, 2, 0, 0],
            vec![2, 1, 2, 2, 2, 0, 0],
        ]
    ]
}

#[derive(Debug)]
struct TeamRegistration {
    registration_token: String,
    expected_players: u8,
    registered_players: Vec<String>,
}

impl TeamRegistration {
    fn new(token: String, expected_players: u8) -> Self {
        Self {
            registration_token: token,
            expected_players,
            registered_players: Vec::new(),
        }
    }
}

fn handle_client(
    mut stream: TcpStream,
    team_state: Arc<Mutex<Option<TeamRegistration>>>,
    chelou64steps: Arc<Vec<String>>,
) {
    match receive_encoded_message(&mut stream) {
        ClientRequest::RegisterTeam { name: _ } => {
            let mut team_guard = team_state.lock().unwrap();
            if team_guard.is_some() {
                let response = ServerResponse::RegisterTeamResult(
                    RegisterTeamResult::Err(RegistrationError::AlreadyRegistered),
                );
                let response_bytes = serde_json::to_vec(&response).unwrap();
                send_encoded_message(&mut stream, response_bytes);
            } else {
                let token = "abcdefghi".to_string();
                let expected_players = 1;
                *team_guard = Some(TeamRegistration::new(token.clone(), expected_players));

                let response = ServerResponse::RegisterTeamResult(RegisterTeamResult::Ok {
                    expected_players,
                    registration_token: token,
                });
                let response_bytes = serde_json::to_vec(&response).unwrap();
                send_encoded_message(&mut stream, response_bytes);
                println!("Équipe enregistrée avec succès");
            }
        }

        ClientRequest::SubscribePlayer {
            name,
            registration_token,
        } => {
            let mut team_guard = team_state.lock().unwrap();
            if let Some(ref mut team) = *team_guard {
                if team.registration_token != registration_token {
                    let response = ServerResponse::SubscribePlayerResult(
                        SubscribePlayerResult::Err(RegistrationError::InvalidRegistrationToken),
                    );
                    let response_bytes = serde_json::to_vec(&response).unwrap();
                    send_encoded_message(&mut stream, response_bytes);
                } else if team.registered_players.len() as u8 >= team.expected_players {
                    let response = ServerResponse::SubscribePlayerResult(
                        SubscribePlayerResult::Err(RegistrationError::TooManyPlayers),
                    );
                    let response_bytes = serde_json::to_vec(&response).unwrap();
                    send_encoded_message(&mut stream, response_bytes);
                } else {
                    team.registered_players.push(name.clone());
                    let response = ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Ok);
                    let response_bytes = serde_json::to_vec(&response).unwrap();
                    send_encoded_message(&mut stream, response_bytes);
                    println!("Joueur '{}' enregistré.", name);

                    let mut player_stream = stream;
                    let chelou64steps = Arc::clone(&chelou64steps);
                    thread::spawn(move || {
                        println!("Thread joueur démarré pour : {}", name);
                        println!("Envoi de la première vue radar");
                        let radar_view1 = ServerResponse::RadarView(
                            chelou64steps[0].clone(),
                        );
                        let response_bytes =
                            serde_json::to_vec(&radar_view1).unwrap();
                        send_encoded_message(&mut player_stream, response_bytes);

                        let mut nb_actions = 0;
                        loop {
                            match receive_encoded_message(&mut player_stream) {
                                ClientRequest::Action(action) => {
                                    println!(
                                        "Reçu une action du joueur {} : {:?}",
                                        name, action
                                    );
                                    match action {
                                        Action::MoveTo(RelativeDirection::Left) => {
                                            println!("Le joueur {} a bougé à gauche", name);
                                            if nb_actions == 1 {
                                                println!("Le joueur a trouvé l'objectif !");
                                                let radar_view3 = ServerResponse::RadarView(
                                                    chelou64steps[2].clone(),
                                                );
                                                let response_bytes =
                                                    serde_json::to_vec(&radar_view3).unwrap();
                                                send_encoded_message(&mut player_stream, response_bytes);
                                                break;
                                            }

                                            let radar_view2 = ServerResponse::RadarView(
                                                chelou64steps[1].clone(),
                                            );
                                            let response_bytes =
                                                serde_json::to_vec(&radar_view2).unwrap();
                                            send_encoded_message(&mut player_stream, response_bytes);

                                            nb_actions += 1;
                                        }
                                        _ => {
                                            println!("Le joueur {} a tenté une action invalide", name);
                                            eprintln!(
                                                "Requête inattendue dans le thread du joueur {}.",
                                                name
                                            );
                                            break;
                                        }
                                    }
                                }
                                _ => {
                                    eprintln!(
                                        "Requête inattendue dans le thread du joueur {}.",
                                        name
                                    );
                                }
                            }
                        }
                    });
                }
            } else {
                let response = ServerResponse::SubscribePlayerResult(
                    SubscribePlayerResult::Err(RegistrationError::InvalidRegistrationToken),
                );
                let response_bytes = serde_json::to_vec(&response).unwrap();
                send_encoded_message(&mut stream, response_bytes);
            }
        }

        _ => {
            eprintln!("Requête non gérée reçue");
        }
    }
}

fn main() {
    let mut chelou64steps_vec = Vec::new();
    let steps = get_steps();
    for step in steps {
        let radar_data = encoder::encode_radar_data(&step);
        let chelou64 = encoder::encode_chelou64(&radar_data);
        println!("Étape radar view : {}", chelou64);
        chelou64steps_vec.push(chelou64);
    }
    println!("Toutes les étapes : {:?}", chelou64steps_vec);

    let chelou64steps = Arc::new(chelou64steps_vec);

    let address = "127.0.0.1:8778";
    let listener = TcpListener::bind(address).expect("Impossible de lier au port 8778");
    println!("Serveur à l'écoute sur {}", address);

    let team_state = Arc::new(Mutex::new(None));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let team_state = Arc::clone(&team_state);
                let chelou64steps = Arc::clone(&chelou64steps);
                thread::spawn(move || {
                    handle_client(stream, team_state, chelou64steps);
                });
            }
            Err(e) => {
                eprintln!("Erreur lors de l'acceptation d'une connexion: {}", e);
            }
        }
    }
}