use crate::decoder::{decode_chelou64, decode_radar_data};
use crate::logger::send_log;
use crate::logger::LogType::{Insignificant, Mandatory, Medium, Minor};
use crate::maze::{neighbors, CellType, Coord, Maze, Orientation, Player};
use common::{send_encoded_message, Action, ClientRequest, RelativeDirection};
use rand::Rng;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::TcpStream;
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub struct AngleHint {
    pub coord: Coord,
    pub orientation: Orientation,
    pub angle: f32,
}

pub fn receive_radar_view(
    channel_sender: &Sender<String>,
    player_name: &String,
    debug: bool,
    player: &mut Player,
    mut maze: &mut Maze,
    mut goal_cell: &mut Option<Coord>,
    radar_view: &String,
) {
    send_log(
        &channel_sender,
        &*player_name,
        &format!("Vue radar reçue : {}", radar_view),
        Medium,
    );
    let decoded = decode_chelou64(&radar_view, debug);
    if let Ok(data) = decoded {
        process_radar_data(
            data,
            debug,
            &channel_sender,
            &player_name,
            &player,
            &mut maze,
            &mut goal_cell,
        );
    }

    send_log(
        &channel_sender,
        &*player_name,
        &format!("Coordonnées globales du joueur : {:?}", player.coords),
        Minor,
    );
    maze.display(&channel_sender, &*player_name, 50, &player);
    // send_log(&channel_sender, &*player_tag, &format!("Maze actuel : {:?}", maze.cells));
}

pub fn process_radar_data(
    data: Vec<u8>,
    debug: bool,
    channel_sender: &Sender<String>,
    player_tag: &str,
    player: &Player,
    maze: &mut Maze,
    goal_cell: &mut Option<Coord>,
) {
    if let Ok((radar_text, grid)) = decode_radar_data(&data, debug) {
        send_log(channel_sender, player_tag, &radar_text, Minor);
        for i in 0..7 {
            for j in 0..7 {
                send_log(
                    channel_sender,
                    player_tag,
                    &format!("Cellule ({}, {}) : {}\n", i, j, grid[i][j]),
                    Insignificant,
                );

                // On obtient les coordonnées globales
                let (global_x, global_y) = local_to_global(
                    channel_sender.clone(), // si tu préfères, tu peux passer &*channel_sender
                    player_tag,
                    player,
                    j as i16 - 3,
                    3 - i as i16,
                );
                send_log(
                    channel_sender,
                    player_tag,
                    &format!("Coordonnées globales : ({};{})", global_x, global_y),
                    Insignificant,
                );

                // On détermine le type de cellule
                let cell_type = match grid[i][j] {
                    0 => CellType::Unknown,
                    1 => CellType::Free,
                    2 => CellType::Wall,
                    5 => CellType::Ally,
                    6 => CellType::Enemy,
                    10 => CellType::Goal,
                    _ => CellType::Unknown,
                };

                // Si c'est l'objectif, on met à jour `goal_cell`
                if cell_type == CellType::Goal {
                    send_log(
                        channel_sender,
                        player_tag,
                        &format!("!!! Objectif trouvé en {:?} !!!", (global_x, global_y)),
                        Mandatory,
                    );
                    *goal_cell = Some(Coord {
                        x: global_x,
                        y: global_y,
                    });
                }

                if cell_type == CellType::Ally {
                    send_log(
                        channel_sender,
                        player_tag,
                        &format!("!!! Allié trouvé en {:?} !!!", (global_x, global_y)),
                        Mandatory,
                    );
                }

                if cell_type == CellType::Enemy {
                    send_log(
                        channel_sender,
                        player_tag,
                        &format!("!!! Ennemi trouvé en {:?} !!!", (global_x, global_y)),
                        Mandatory,
                    );
                }

                // On met à jour le Maze
                maze.set_cell(
                    Coord {
                        x: global_x,
                        y: global_y,
                    },
                    cell_type,
                );
            }
        }
        maze.display_numbers(channel_sender, player_tag, 3);
    }
}

pub fn find_nearest_frontier(
    maze: &Maze,
    start: Coord,
    channel_sender: &Sender<String>,
    player_name: &str,
) -> Option<Coord> {
    use std::collections::{HashSet, VecDeque};
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        // Vérifier si c'est une frontière (case Unknown adjacente à une case Free)
        for neighbor in neighbors(current) {
            send_log(
                channel_sender,
                player_name,
                &format!(
                    "Test Voisin : {:?} (type: {:?})",
                    neighbor,
                    maze.get_cell(&neighbor)
                ),
                Insignificant,
            );
            if maze.get_cell(&neighbor) == CellType::Unknown {
                send_log(
                    channel_sender,
                    player_name,
                    &format!(
                        "Cellule unknown trouvée : {:?} (type: {:?})",
                        neighbor,
                        maze.get_cell(&neighbor)
                    ),
                    Insignificant,
                );
                for adj in neighbors(neighbor) {
                    if (maze.get_cell(&adj) == CellType::Free
                        || maze.get_cell(&adj) == CellType::Ally)
                        && adj != start
                    {
                        send_log(
                            channel_sender,
                            player_name,
                            &format!(
                                "Frontière trouvée : {:?} (type: {:?})",
                                neighbor,
                                maze.get_cell(&neighbor)
                            ),
                            Insignificant,
                        );
                        if neighbor.x % 2 == 0 && neighbor.y % 2 == 0 {
                            return Some(neighbor);
                        }
                        return Some(adj);
                    }
                }
            }
            // Si ce n'est pas une frontière, continuer l'exploration BFS
            if (maze.get_cell(&neighbor) == CellType::Free
                || maze.get_cell(&neighbor) == CellType::Ally)
                && !visited.contains(&neighbor)
            {
                send_log(
                    channel_sender,
                    player_name,
                    &format!(
                        "Ajout du voisin en visité : {:?} (type: {:?})",
                        neighbor,
                        maze.get_cell(&neighbor)
                    ),
                    Insignificant,
                );
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    send_log(
        channel_sender,
        player_name,
        "AUCUNE FRONTIERE TROUVEE, THREAD MIGHT PANIC.",
        Mandatory,
    );
    None
}

pub fn compute_direction(
    player_orientation: Orientation,
    wanted_direction: Orientation,
) -> RelativeDirection {
    // si je suis orienté vers l'est et que je veux aller vers le nord, je dois tourner à gauche
    // si je suis orienté vers l'est et que je veux aller vers le sud, je dois tourner à droite
    // etc...
    match player_orientation {
        Orientation::North => match wanted_direction {
            Orientation::North => RelativeDirection::Front,
            Orientation::East => RelativeDirection::Right,
            Orientation::South => RelativeDirection::Back,
            Orientation::West => RelativeDirection::Left,
        },
        Orientation::East => match wanted_direction {
            Orientation::North => RelativeDirection::Left,
            Orientation::East => RelativeDirection::Front,
            Orientation::South => RelativeDirection::Right,
            Orientation::West => RelativeDirection::Back,
        },
        Orientation::South => match wanted_direction {
            Orientation::North => RelativeDirection::Back,
            Orientation::East => RelativeDirection::Left,
            Orientation::South => RelativeDirection::Front,
            Orientation::West => RelativeDirection::Right,
        },
        Orientation::West => match wanted_direction {
            Orientation::North => RelativeDirection::Right,
            Orientation::East => RelativeDirection::Back,
            Orientation::South => RelativeDirection::Left,
            Orientation::West => RelativeDirection::Front,
        },
    }
}

pub fn move_to_next_step_random(
    player_tag: &str,
    channel_sender: &Sender<String>,
    tcp_stream: &mut TcpStream,
) {
    let mut rng = rand::rng();
    let random_direction = match rng.random_range(0..4) {
        0 => RelativeDirection::Front,
        1 => RelativeDirection::Right,
        2 => RelativeDirection::Back,
        3 => RelativeDirection::Left,
        _ => RelativeDirection::Front,
    };

    let action = Action::MoveTo(random_direction);
    let client_request = ClientRequest::Action(action.clone());
    let json_message = serde_json::to_vec(&client_request).unwrap();

    send_log(
        channel_sender,
        player_tag,
        &format!(
            "Déplacement : {:?} (message : {:?})",
            random_direction.clone(),
            client_request
        ),
        Medium,
    );
    send_encoded_message(tcp_stream, json_message);
}

pub fn move_to_next_step(
    player: &mut Player,
    player_tag: &str,
    next_step: Coord,
    channel_sender: &Sender<String>,
    tcp_stream: &mut TcpStream,
    last_action: &mut Option<Action>,
    last_player_coords: &mut Coord,
    last_player_orientation: &mut Orientation,
) {
    send_log(
        channel_sender,
        player_tag,
        &format!("Trying to move to next step : {:?}", next_step),
        Medium,
    );

    // log calcul du vecteur
    send_log(
        channel_sender,
        player_tag,
        &format!(
            "Calcul du vecteur : next_step = {:?} et player.coords = {:?}",
            next_step, player.coords
        ),
        Insignificant,
    );

    // ex : next_step = Coord { x: -4, y: 2 } et player.coords = Coord { x: -4, y: 4 }
    let dx = next_step.x - player.coords.x;
    let dy = next_step.y - player.coords.y;
    // dx = 0, dy = -2
    send_log(
        channel_sender,
        player_tag,
        &format!("Vecteur calculé : dx = {}, dy = {}", dx, dy),
        Insignificant,
    );

    // [2025-01-16 11:02:02] [Joueur_1] Chemin filtré : [Coord { x: -4, y: 2 }, Coord { x: -2, y: 2 }, Coord { x: -2, y: 4 }]
    // [2025-01-16 11:02:02] [Joueur_1] BEFORE MOVE : Coordonnées du joueur : Coord { x: -4, y: 4 }
    // [2025-01-16 11:02:02] [Joueur_1] BEFORE MOVE : Orientation du joueur : East
    // [2025-01-16 11:02:02] [Joueur_1] Déplacement : Left (message : Action(MoveTo(Left)))
    // [2025-01-16 11:02:02] [Joueur_1] Coordonnées mises à jour : Coord { x: -4, y: 6 }
    // [2025-01-16 11:02:02] [Joueur_1] AFTER MOVE : Coordonnées du joueur : Coord { x: -4, y: 6 }
    // [2025-01-16 11:02:02] [Joueur_1] AFTER MOVE : Orientation du joueur : North

    *last_player_orientation = player.orientation.clone();
    let mut direction = RelativeDirection::Front;
    if dx == 2 && dy == 0 {
        send_log(
            channel_sender,
            player_tag,
            &format!(
                "Déplacement vers est, orientation du joueur : {:?}",
                player.orientation
            ),
            Insignificant,
        );
        send_log(
            channel_sender,
            player_tag,
            "Direction Possibles selon orientation joueur :\nNord: Direction Left\nEst: Direction Front\nSud: Direction Right\nOuest: Direction Back",
            Insignificant
        );
        direction = compute_direction(player.orientation.clone(), Orientation::East);
        send_log(
            channel_sender,
            player_tag,
            &format!("Direction calculée pour joueur avec orientation initiale de {:?} et une orientation voulue à l'Est : {:?}", player.orientation, direction),
            Insignificant
        );
        player.orientation = Orientation::East;
    } else if dx == -2 && dy == 0 {
        send_log(
            channel_sender,
            player_tag,
            &format!(
                "Déplacement vers ouest, orientation du joueur : {:?}",
                player.orientation
            ),
            Insignificant,
        );
        send_log(
            channel_sender,
            player_tag,
            "Direction Possibles selon orientation joueur :\nNord: Direction Right\nEst: Direction Back\nSud: Direction Left\nOuest: Direction Front",
            Insignificant
        );
        direction = compute_direction(player.orientation.clone(), Orientation::West);
        send_log(
            channel_sender,
            player_tag,
            &format!("Direction calculée pour joueur avec orientation initiale de {:?} et une orientation voulue à l'Ouest : {:?}", player.orientation, direction),
            Insignificant
        );
        player.orientation = Orientation::West;
    } else if dx == 0 && dy == 2 {
        send_log(
            channel_sender,
            player_tag,
            &format!(
                "Déplacement vers nord, orientation du joueur : {:?}",
                player.orientation
            ),
            Insignificant,
        );
        send_log(
            channel_sender,
            player_tag,
            "Direction Possibles selon orientation joueur :\nNord: Direction Front\nEst: Direction Left\nSud: Direction Back\nOuest: Direction Right",
            Insignificant
        );
        direction = compute_direction(player.orientation.clone(), Orientation::North);
        send_log(
            channel_sender,
            player_tag,
            &format!("Direction calculée pour joueur avec orientation initiale de {:?} et une orientation voulue au Nord : {:?}", player.orientation, direction),
            Insignificant
        );
        player.orientation = Orientation::North;
    } else if dx == 0 && dy == -2 {
        // déplacement vers sud, orientation du joueur :
        send_log(
            channel_sender,
            player_tag,
            &format!(
                "Déplacement vers sud, orientation du joueur : {:?}",
                player.orientation
            ),
            Insignificant,
        );
        // log des directions calculables possibles
        send_log(
            channel_sender,
            player_tag,
            "Direction Possibles selon orientation joueur :\nNord: Direction Back\nEst: Direction Right\nSud: Direction Front\nOuest: Direction Left",
            Insignificant
        );
        direction = compute_direction(player.orientation.clone(), Orientation::South);
        // direction calculée :
        send_log(
            channel_sender,
            player_tag,
            &format!("Direction calculée pour joueur avec orientation initiale de {:?} et une orientation voulue au Sud : {:?}", player.orientation, direction),
            Insignificant
        );
        player.orientation = Orientation::South;
    } else {
        send_log(
            channel_sender,
            player_tag,
            "Coordonnée invalide pour le déplacement",
            Mandatory,
        );
    };

    let action = Action::MoveTo(direction);
    let client_request = ClientRequest::Action(action.clone());
    let json_message = serde_json::to_vec(&client_request).unwrap();

    send_log(
        channel_sender,
        player_tag,
        &format!(
            "Déplacement : {:?} (message : {:?})",
            direction.clone(),
            client_request
        ),
        Medium,
    );
    send_encoded_message(tcp_stream, json_message);

    *last_player_coords = player.coords.clone();
    *last_action = Some(action);
    player.coords = next_step;
    send_log(
        channel_sender,
        player_tag,
        &format!("Coordonnées mises à jour : {:?}", player.coords),
        Medium,
    );
    // player.coords = Coord { x: 24, y: 10 }
}

pub fn find_path_to_goal(
    start: Coord,
    goal: Coord,
    maze: &Maze,
    channel_sender: &Sender<String>,
    player_tag: &str,
) -> Vec<Coord> {
    let mut queue: VecDeque<Coord> = VecDeque::new();
    let mut visited: HashSet<Coord> = HashSet::new();
    let mut parent_map: HashMap<Coord, Coord> = HashMap::new();

    // Initialisation de la file avec la position de départ
    queue.push_back(start);
    visited.insert(start);

    send_log(
        channel_sender,
        player_tag,
        &format!(
            "Début de la recherche du chemin : start={:?}, goal={:?}",
            start, goal
        ),
        Minor,
    );

    while let Some(current) = queue.pop_front() {
        // Si on atteint l'objectif, reconstruire le chemin
        if current == goal {
            let mut path = vec![current];
            while let Some(&parent) = parent_map.get(&path.last().unwrap()) {
                path.push(parent);
            }
            path.reverse(); // On retourne le chemin pour partir du start

            send_log(
                channel_sender,
                player_tag,
                &format!("Chemin trouvé : {:?}", path),
                Minor,
            );
            if path.is_empty() {
                send_log(
                    channel_sender,
                    player_tag,
                    "!! WARNING !! : Aucun chemin trouvé (avant filtrage).",
                    Mandatory,
                );
            }

            // Filtrer les coordonnées impaires et retirer le start
            let filtered_path: Vec<Coord> = path
                .into_iter()
                .skip(1) // Supprimer le start
                .filter(|coord| coord.x % 2 == 0 && coord.y % 2 == 0) // Conserver uniquement les coordonnées paires
                .collect();

            send_log(
                channel_sender,
                player_tag,
                &format!("Chemin filtré : {:?}", filtered_path),
                Medium,
            );
            if filtered_path.is_empty() {
                send_log(
                    channel_sender,
                    player_tag,
                    "!! WARNING !! : Aucun chemin trouvé (après filtrage).",
                    Mandatory,
                );
            }
            return filtered_path;
        }

        // Explorer les voisins
        for neighbor in neighbors(current) {
            // Vérifier qu'on ne l'a pas déjà visité
            if visited.contains(&neighbor) {
                continue;
            }

            if maze.get_cell(&neighbor) == CellType::Wall {
                continue;
            }

            // Si on passe ces checks, c'est un voisin valable
            visited.insert(neighbor);
            parent_map.insert(neighbor, current);
            queue.push_back(neighbor);

            send_log(
                channel_sender,
                player_tag,
                &format!(
                    "Ajout du voisin à la file : {:?} (type: {:?})",
                    neighbor,
                    maze.get_cell(&neighbor)
                ),
                Insignificant,
            );
        }
    }

    // Si aucun chemin n'est trouvé, retourner un chemin vide
    send_log(
        channel_sender,
        player_tag,
        "Aucun chemin trouvé.",
        Mandatory,
    );
    Vec::new()
}

pub fn local_to_global(
    channel_sender: Sender<String>,
    player_tag: &str,
    player: &Player,
    x: i16,
    y: i16,
) -> (i16, i16) {
    // -2 et -1 NORTH = -2,-1
    // -2 et -1 EAST = -1,2
    // -2 et -1 SOUTH = 2,1
    // -2 et -1 WEST = 1,-2
    match player.orientation {
        Orientation::North => {
            // log : (0,0) + (-2,-1) = (-2,-1)
            send_log(
                &channel_sender,
                &*player_tag,
                &format!(
                    "log : {:?} + {:?} = {:?}",
                    player.coords,
                    (x, y),
                    (player.coords.x + x, player.coords.y + y)
                ),
                Insignificant,
            );
            (player.coords.x + x, player.coords.y + y)
        }
        Orientation::East => {
            // log : (0,0) + (-2,-1) = (-1,2)
            send_log(
                &channel_sender,
                &*player_tag,
                &format!(
                    "log : {:?} + {:?} = {:?}",
                    player.coords,
                    (y, -x),
                    (player.coords.x + y, player.coords.y - x)
                ),
                Insignificant,
            );
            (player.coords.x + y, player.coords.y - x)
        }
        Orientation::South => {
            // log : (0,0) + (-2,-1) = (2,1)
            send_log(
                &channel_sender,
                &*player_tag,
                &format!(
                    "log : {:?} + {:?} = {:?}",
                    player.coords,
                    (-x, -y),
                    (player.coords.x - x, player.coords.y - y)
                ),
                Insignificant,
            );
            (player.coords.x - x, player.coords.y - y)
        }
        Orientation::West => {
            // log : (0,0) + (-2,-1) = (1,-2)
            send_log(
                &channel_sender,
                &*player_tag,
                &format!(
                    "log : {:?} + {:?} = {:?}",
                    player.coords,
                    (-y, x),
                    (player.coords.x - y, player.coords.y + x)
                ),
                Insignificant,
            );
            (player.coords.x - y, player.coords.y + x)
        }
    }
}

#[cfg(test)]
mod solver_tests {
    use super::*;

    #[test]
    fn test_compute_direction() {
        assert_eq!(
            compute_direction(Orientation::North, Orientation::North),
            RelativeDirection::Front
        );
        assert_eq!(
            compute_direction(Orientation::North, Orientation::East),
            RelativeDirection::Right
        );
        assert_eq!(
            compute_direction(Orientation::North, Orientation::South),
            RelativeDirection::Back
        );
        assert_eq!(
            compute_direction(Orientation::North, Orientation::West),
            RelativeDirection::Left
        );

        assert_eq!(
            compute_direction(Orientation::East, Orientation::North),
            RelativeDirection::Left
        );
        assert_eq!(
            compute_direction(Orientation::East, Orientation::East),
            RelativeDirection::Front
        );
        assert_eq!(
            compute_direction(Orientation::East, Orientation::South),
            RelativeDirection::Right
        );
        assert_eq!(
            compute_direction(Orientation::East, Orientation::West),
            RelativeDirection::Back
        );

        assert_eq!(
            compute_direction(Orientation::South, Orientation::North),
            RelativeDirection::Back
        );
        assert_eq!(
            compute_direction(Orientation::South, Orientation::East),
            RelativeDirection::Left
        );
        assert_eq!(
            compute_direction(Orientation::South, Orientation::South),
            RelativeDirection::Front
        );
        assert_eq!(
            compute_direction(Orientation::South, Orientation::West),
            RelativeDirection::Right
        );

        assert_eq!(
            compute_direction(Orientation::West, Orientation::North),
            RelativeDirection::Right
        );
        assert_eq!(
            compute_direction(Orientation::West, Orientation::East),
            RelativeDirection::Back
        );
        assert_eq!(
            compute_direction(Orientation::West, Orientation::South),
            RelativeDirection::Left
        );
        assert_eq!(
            compute_direction(Orientation::West, Orientation::West),
            RelativeDirection::Front
        );
    }

    #[test]
    fn test_local_to_global() {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let player = Player {
            coords: Coord { x: 0, y: 0 },
            orientation: Orientation::East,
        };
        let (x, y) = local_to_global(sender.clone(), "Test", &player, -2, -1);
        assert_eq!(x, -1);
        assert_eq!(y, 2);
    }

    #[test]
    fn test_find_nearest_frontier() {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut maze = Maze::new();
        maze.set_cell(Coord { x: 0, y: 0 }, CellType::Free);
        maze.set_cell(Coord { x: 0, y: 1 }, CellType::Wall);
        maze.set_cell(Coord { x: 1, y: 0 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: 0 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -1 }, CellType::Free);
        maze.set_cell(Coord { x: -1, y: -1 }, CellType::Wall);
        maze.set_cell(Coord { x: 1, y: -1 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -2 }, CellType::Free);
        maze.set_cell(Coord { x: 1, y: -2 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: -2 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -3 }, CellType::Free);
        maze.set_cell(Coord { x: 1, y: -3 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: -3 }, CellType::Wall);

        let frontier = find_nearest_frontier(&maze, Coord { x: 0, y: 0 }, &sender, "Test");
        assert_eq!(frontier, Some(Coord { x: 0, y: -4 }));
    }

    #[test]
    fn test_find_path_to_goal() {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut maze = Maze::new();
        maze.set_cell(Coord { x: 0, y: 0 }, CellType::Free);
        maze.set_cell(Coord { x: 0, y: 1 }, CellType::Wall);
        maze.set_cell(Coord { x: 1, y: 0 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: 0 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -1 }, CellType::Free);
        maze.set_cell(Coord { x: -1, y: -1 }, CellType::Wall);
        maze.set_cell(Coord { x: 1, y: -1 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -2 }, CellType::Free);
        maze.set_cell(Coord { x: 1, y: -2 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: -2 }, CellType::Wall);
        maze.set_cell(Coord { x: 0, y: -3 }, CellType::Free);
        maze.set_cell(Coord { x: 1, y: -3 }, CellType::Wall);
        maze.set_cell(Coord { x: -1, y: -3 }, CellType::Wall);

        let path = find_path_to_goal(
            Coord { x: 0, y: 0 },
            Coord { x: 0, y: -4 },
            &maze,
            &sender,
            "Test",
        );
        assert_eq!(path, vec![Coord { x: 0, y: -2 }, Coord { x: 0, y: -4 }]);
    }
}
