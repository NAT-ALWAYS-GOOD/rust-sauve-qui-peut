//! Module principal du serveur. Ce module gère l'enregistrement des équipes et l'inscription des joueurs,
//! et leur délivre des vues radar encodées en chelou64.
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use common::{receive_encoded_message, send_encoded_message, ClientRequest, RegisterTeamResult, RegistrationError, ServerResponse, SubscribePlayerResult, Action, RelativeDirection};

mod encoder;

/// Renvoie un ensemble prédéfini d'étapes sous forme de grilles 7x7.
///
/// Chaque étape est représentée par un vecteur de vecteurs d'octets, où chaque nombre
/// correspond à un type d'élément (par exemple, 2 pour mur, 0 pour inconnu, 10 pour objectif, 1 pour passage libre, etc.).
///
/// # Retour
///
/// Un vecteur contenant plusieurs grilles (étapes) pour simuler des vues radar.
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

/// Structure représentant l'état d'enregistrement d'une équipe.
///
/// Contient le token d'enregistrement, le nombre de joueurs attendus, ainsi que
/// la liste des joueurs déjà enregistrés.
#[derive(Debug)]
struct TeamRegistration {
    registration_token: String,
    expected_players: u8,
    registered_players: Vec<String>,
}

impl TeamRegistration {
    /// Crée une nouvelle instance de `TeamRegistration`.
    ///
    /// # Arguments
    ///
    /// * `token` - Le token d'enregistrement attribué à l'équipe.
    /// * `expected_players` - Le nombre de joueurs attendus pour l'équipe.
    ///
    /// # Retour
    ///
    /// Une instance initialisée de `TeamRegistration` avec une liste vide de joueurs.
    fn new(token: String, expected_players: u8) -> Self {
        Self {
            registration_token: token,
            expected_players,
            registered_players: Vec::new(),
        }
    }
}

/// Gère la connexion d'un client.
///
/// Cette fonction lit la requête envoyée par le client, qui peut être soit une demande
/// d'enregistrement d'équipe, soit une demande d'inscription d'un joueur. Selon la requête,
/// la fonction envoie la réponse appropriée et, dans le cas d'une inscription réussie,
/// démarre un thread dédié pour gérer la communication ultérieure avec le joueur.
///
/// # Arguments
///
/// * `stream` - Le flux TCP de communication avec le client.
/// * `team_state` - État partagé (protégé par un Mutex) contenant les informations d'enregistrement de l'équipe.
/// * `chelou64steps` - Ensemble de vues radar encodées en chelou64, partagé entre les threads.
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

/// Point d'entrée du serveur.
///
/// La fonction effectue les opérations suivantes :
/// - Génère les différentes étapes radar en encodant des grilles via le module `encoder`.
/// - Lie le serveur à l'adresse "127.0.0.1:8778" et attend les connexions entrantes.
/// - Pour chaque connexion, un thread est lancé afin de traiter la requête via la fonction `handle_client`.
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