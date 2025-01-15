use std::collections::{HashMap, HashSet, VecDeque};
use std::net::TcpStream;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use common::{ClientRequest, ServerResponse, RegisterTeamResult, SubscribePlayerResult, send_encoded_message, receive_encoded_message, RelativeDirection, Action};
use crate::LogType::{Insignificant, Major, Mandatory, Medium, Minor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Unknown, // Pas encore exploré
    Free,    // 1 : passage libre
    Wall,    // 2 : mur
    Goal,    // 10 : objectif
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    North,
    East,
    South,
    West,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}
struct Player {
    coords: Coord,
    orientation: Orientation,
}

struct Maze {
    pub cells: HashMap<Coord, CellType>,
}

impl Maze {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new()
        }
    }

    pub fn set_cell(&mut self, coord: Coord, cell_type: CellType) {
        // Une cellule ne peut pas changer de type
        if self.cells.contains_key(&coord) {
            match self.cells.get(&coord) {
                Some(cell) if *cell != CellType::Unknown => return,
                _ => (),
            }
        }
        self.cells.insert(coord, cell_type);
    }

    pub fn get_cell(&self, coord: &Coord) -> CellType {
        let cell_type : Option<&CellType> = self.cells.get(coord);
        match cell_type {
            Some(cell) => *cell,
            // None => if coord.x.abs() % 2 == 1 && coord.y.abs() % 2 == 1 {
            //     CellType::Wall
            // } else {
            //     CellType::Unknown
            // },
            None => CellType::Unknown
        }
    }

    pub fn display_numbers(&self, channel_sender: &Sender<String>, tag: &str, radius: i16) {
        let mut output = String::new();
        output.push_str("=== Maze Nombres 7x7 ===\n");

        for y in (-radius..=radius).rev() {
            for x in -radius..=radius {
                let coord = Coord { x, y };
                let cell = self.get_cell(&coord);
                let cell_str = match cell {
                    CellType::Unknown => "0",
                    CellType::Free    => "1",
                    CellType::Wall    => "2",
                    CellType::Goal    => "10",
                };
                output.push_str(cell_str);
            }
            output.push_str("\n");
        }

        send_log(channel_sender, tag, &output, Minor);
    }

    pub fn display(&self, channel_sender: &Sender<String>, tag: &str, radius: i16) {
        let mut output = String::new();
        output.push_str("=== Maze 7x7 ===\n");

        for y in (-radius..=radius).rev() {
            for x in -radius..=radius {
                let coord = Coord { x, y };
                let cell = self.get_cell(&coord);
                let cell_str = match cell {
                    CellType::Unknown => "🟫",
                    CellType::Free    => "⬜️",
                    CellType::Wall    => "⬛️",
                    CellType::Goal    => "🎯",
                };
                output.push_str(cell_str);
            }
            output.push_str("\n");
        }

        send_log(channel_sender, tag, &output, Minor);
    }
}

enum LogType {
    Insignificant,
    Minor,
    Medium,
    Major,
    Mandatory,
}

fn send_log(channel_sender: &Sender<String>, tag: &str, message: &str, log_type: LogType) {
    let log_type_level = Major;

    if log_type as i32 >= log_type_level as i32 {
        channel_sender.send(format!("[{}] : {}", tag, message)).unwrap();
    }
}

fn register_team(channel_sender: &Sender<String>, server_address: &str) -> Option<String> {
    println!("Tentative de connexion au serveur {} pour l'équipe...", server_address);
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
                        Medium
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

fn decode_chelou64(input: &str, _debug: bool) -> Result<Vec<u8>, String> {
    let len = input.len();

    if len % 4 == 1 {
        return Err("Taille invalide (4n+1) pour cet encodage 6 bits/char".to_string());
    }

    fn decode_6bits_char(c: char) -> Option<u8> {
        match c {
            'a'..='z' => Some(c as u8 - b'a'),                         // 0..25
            'A'..='Z' => Some(26 + (c as u8 - b'A')),                  // 26..51
            '0'..='9' => Some(52 + (c as u8 - b'0')),                  // 52..61
            '+' => Some(62),
            '/' => Some(63),
            _ => None,
        }
    }

    let mut output = Vec::new();
    let mut i = 0;

    while i < len {
        // Combien de caractères dans ce bloc ?
        let remaining = len - i;
        let block_size = remaining.min(4);

        // On récupère la valeur 6 bits de chaque caractère.
        // S'il en manque, on complète "virtuellement" par des 0.
        let mut chunk = [0u8; 4];
        for c in 0..block_size {
            let chr = input.chars().nth(i + c).unwrap();
            match decode_6bits_char(chr) {
                Some(val) => chunk[c] = val,
                None => {
                    return Err(format!("Caractère non autorisé: '{}'", chr));
                }
            }
        }

        // On produit toujours le 1er octet
        let first_byte = (chunk[0] << 2) | (chunk[1] >> 4);
        output.push(first_byte);

        // 2e octet si on a au moins 3 caractères
        if block_size >= 3 {
            let second_byte = ((chunk[1] & 0x0F) << 4) | (chunk[2] >> 2);
            output.push(second_byte);
        }

        // 3e octet si on a 4 caractères
        if block_size == 4 {
            let third_byte = ((chunk[2] & 0x03) << 6) | chunk[3];
            output.push(third_byte);
        }

        i += block_size;
    }

    Ok(output)
}

fn push_str_debug(output: &mut String, debug: bool, message: &str) {
    if debug {
        output.push_str(message);
    }
}

fn decode_radar_data(decoded: &[u8], debug: bool) -> Result<(String, Vec<Vec<u8>>),String> {
    if decoded.len() != 11 {
        return Err(format!(
            "Données incorrectes : {} octets reçus au lieu de 11",
            decoded.len()
        ));
    }

    let mut output = String::new();

    // Affichage des données brutes
    push_str_debug(&mut output, debug, "=== Données brutes (6 premiers octets) ===\n");
    for i in 0..6 {
        push_str_debug(&mut output, debug, &format!("{:08b} ", decoded[i]));
    }
    push_str_debug(&mut output, debug, "\n=== Données brutes (5 derniers octets) ===\n");
    for i in 6..11 {
        push_str_debug(&mut output, debug, &format!("{:02X} ", decoded[i]));
    }
    push_str_debug(&mut output, debug, "\n");

    let horiz_be = [decoded[0], decoded[1], decoded[2], 0];
    let horiz_bits = u32::from_le_bytes(horiz_be);
    push_str_debug(&mut output, debug, &format!("Bits horizontaux (little-endian) : {:032b}\n", horiz_bits));

    let mut horizontal_passages = Vec::with_capacity(12);
    for i in 0..12 {
        let shift = 24 - 2 * (i + 1); // Calculer la position des 2 bits à extraire
        let two_bits = (horiz_bits >> shift) & 0b11; // Extraire les 2 bits
        push_str_debug(&mut output, debug, &format!("\nPassage '-' {} : {:b}", i + 1, two_bits as u8));
        horizontal_passages.push(two_bits as u8);
    }

    let vert_be = [decoded[3], decoded[4], decoded[5], 0];
    let vert_bits = u32::from_le_bytes(vert_be);
    push_str_debug(&mut output, debug, &format!("Bits verticaux (little-endian) : {:032b}\n", vert_bits));

    let mut vertical_passages = Vec::with_capacity(12);
    for i in 0..12 {
        let shift = 24 - 2 * (i + 1);
        let two_bits = (vert_bits >> shift) & 0b11;
        push_str_debug(&mut output, debug, &format!("\nPassage '|' {} : {:b}", i + 1, two_bits as u8));
        vertical_passages.push(two_bits as u8);
    }

    let cell_bytes = [
        decoded[6], decoded[7],
        decoded[8], decoded[9],
        decoded[10],
    ];

    let cells = handle_cells(&mut output, cell_bytes, debug);

    let mut grid = vec![vec!["•".to_string(); 7]; 7];
    let mut internal_grid : Vec<Vec<u8>> = vec![vec![2; 7]; 7];

    // Passages horizontaux
    let hcoords = [
        (0, 1), (0, 3), (0, 5),
        (2, 1), (2, 3), (2, 5),
        (4, 1), (4, 3), (4, 5),
        (6, 1), (6, 3), (6, 5),
    ];

    // Passages verticaux
    let vcoords = [
        (1, 0), (1, 2), (1, 4), (1, 6),
        (3, 0), (3, 2), (3, 4), (3, 6),
        (5, 0), (5, 2), (5, 4), (5, 6),
    ];

    // Coordonnées cellules (3×3)
    let ccoords = [
        (1, 1), (1, 3), (1, 5),
        (3, 1), (3, 3), (3, 5),
        (5, 1), (5, 3), (5, 5),
    ];

    // On remplit la grille avec les passages horizontaux
    for (i, &(r, c)) in hcoords.iter().enumerate() {
        let p = horizontal_passages[i];
        grid[r][c] = match p {
            0 => "#".to_string(), // Undefined
            1 => " ".to_string(), // Open
            2 => "-".to_string(), // Wall
            _ => "#".to_string(),
        };
        internal_grid[r][c] = p;
    }

    // On remplit la grille avec les passages verticaux
    for (i, &(r, c)) in vcoords.iter().enumerate() {
        let p = vertical_passages[i];
        grid[r][c] = match p {
            0 => "#".to_string(),
            1 => " ".to_string(),
            2 => "|".to_string(),
            _ => "#".to_string(),
        };
        internal_grid[r][c] = p;
    }

    // On remplit la grille avec les 9 cellules
    for (i, &(r, c)) in ccoords.iter().enumerate() {
        let cell_val = decode_cell_symbol(cells[i]);
        grid[r][c] = cell_val.clone();
        internal_grid[r][c] = match cells[i] {
            0b1111 => 0,
            0b1000 => 10,
            0b1001 => 10,
            0b1010 => 10,
            0b1011 => 10,
            _ => 1,
        }

    }

    // Affichage de la grille
    output.push_str(&"\n=== Grille 7x7 ===\n".to_string());
    for row in 0..7 {
        let row_str = grid[row].join("");
        output.push_str(&format!("{}\n", row_str));
    }

    // Affichage de l'internal_grid
    output.push_str(&"\n=== Grille interne 7x7 ===\n".to_string());
    for row in 0..7 {
        let row_str = internal_grid[row].iter().map(|&val| format!("{}", val)).collect::<Vec<_>>().join("");
        output.push_str(&format!("{}\n", row_str));
    }

    Ok((output,internal_grid))
}

fn handle_cells(output: &mut String, cell_bytes: [u8; 5], debug: bool) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(10);
    for byte in cell_bytes {
        let high = (byte >> 4) & 0x0F;
        let low = byte & 0x0F;
        nibbles.push(high);
        nibbles.push(low);
    }

    push_str_debug(output, debug, "\n=== Les 10 nibbles (4 bits) ===\n");
    let nibs_str = nibbles
        .iter()
        .map(|val| format!("{:X}", val))
        .collect::<Vec<_>>()
        .join(",");
    push_str_debug(output, debug, &format!("{}\n", nibs_str));

    // On ignore le 10e nibble qui est un 0 inutile
    let cells = &nibbles[..9]; // slices des 9 premiers

    push_str_debug(output, debug, "\n=== Cellules (9) ===\n");
    for (i, &val) in cells.iter().enumerate() {
        push_str_debug(output, debug, &format!("Cellule {} => nibble=0x{:X}\n", i + 1, val));
    }

    // Petit affichage pour debug sur les cellules
    push_str_debug(output, debug, "\n=== Cellules (3x3)(remplissage) ===\n");
    for (i, &val) in cells.iter().enumerate() {
        let cell_info = decode_cell_info(val);
        if cell_info != "Rien" && cell_info != "Undefined" {
            output.push_str("\n\nWARNING : CELLULE NON VIDE REPEREE\n\n");
        }
        push_str_debug(output, debug, &format!(
            "Cellule {}: b-{:04b} h-{:X} => \"{}\"\n",
            i + 1,
            val,
            val,
            cell_info
        ));
    }
    push_str_debug(output, debug, "\n");
    cells.to_vec()
}

fn decode_cell_symbol(nibble: u8) -> String {
    if nibble == 0b1111 {
        return "#".to_string();
    }
    let nature = (nibble & 0b1100) >> 2;  // bits 3..2
    let entite = nibble & 0b0011;        // bits 1..0

    let nature_char = match nature {
        0b00 => "",    // Rien
        0b01 => "H",   // Indice
        0b10 => "G",   // Objectif
        0b11 => "?",   // Inconnu
        _ => unreachable!(),
    };
    let entite_char = match entite {
        0b00 => "",    // Aucune entité
        0b01 => "P",   // Allié
        0b10 => "O",   // Ennemi
        0b11 => "M",   // Monstre
        _ => unreachable!(),
    };

    match (nature_char.is_empty(), entite_char.is_empty()) {
        (true, true) => " ".to_string(),
        (false, true) => nature_char.into(),
        (true, false) => entite_char.into(),
        (false, false) => format!("{}/{}", nature_char, entite_char),
    }
}

fn decode_cell_info(nibble: u8) -> String {
    if nibble == 0b1111 {
        return "Undefined".to_string();
    }
    let (nature_str, entite_str) = decode_nature_et_entite(nibble);
    let mut parts = Vec::new();
    if !nature_str.is_empty() {
        parts.push(nature_str);
    }
    if !entite_str.is_empty() {
        parts.push(entite_str);
    }
    if parts.is_empty() {
        "Rien".to_string()
    } else {
        parts.join(" / ")
    }
}

fn decode_nature_et_entite(nibble: u8) -> (String, String) {
    let nature = (nibble & 0b1100) >> 2;
    let entite = nibble & 0b0011;

    let nature_str = match nature {
        0b00 => "",
        0b01 => "Indice",
        0b10 => "Objectif",
        0b11 => "???",   // non précisé => on met ???
        _ => unreachable!(),
    };

    let entite_str = match entite {
        0b00 => "",
        0b01 => "Allié",
        0b10 => "Ennemi",
        0b11 => "Monstre",
        _ => unreachable!(),
    };

    (nature_str.to_string(), entite_str.to_string())
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
            send_log(channel_sender, player_tag, "Enregistrement réussi",Minor);
            true
        }
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Err(error)) => {
            send_log(channel_sender, player_tag, &format!("Erreur lors de l'enregistrement : {:?}", error),Mandatory);
            false
        }
        _ => {
            send_log(channel_sender, player_tag, "Message inattendu reçu lors de l'enregistrement",Mandatory);
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

    let (channel_sender, channel_receiver) = mpsc::channel();

    let reg_token = match register_team(&channel_sender, server_address) {
        Some(token) => token,
        None => {
            println!("Impossible d'enregistrer l'équipe.");
            println!("{}", channel_receiver.recv().unwrap());
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
            index
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

fn neighbors(coord: Coord) -> Vec<Coord> {
    vec![
        Coord { x: coord.x,     y: coord.y + 1 },
        Coord { x: coord.x,     y: coord.y - 1 },
        Coord { x: coord.x - 1, y: coord.y },
        Coord { x: coord.x + 1, y: coord.y }
    ]
}

fn find_nearest_frontier(maze: &Maze, start: Coord, channel_sender: &Sender<String>, player_name: &str) -> Option<Coord> {
    use std::collections::{VecDeque, HashSet};
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
                &format!("Test Voisin : {:?} (type: {:?})", neighbor, maze.get_cell(&neighbor)),
                Insignificant
            );
            if maze.get_cell(&neighbor) == CellType::Unknown {
                send_log(
                    channel_sender,
                    player_name,
                    &format!("Cellule unknown trouvée : {:?} (type: {:?})", neighbor, maze.get_cell(&neighbor)),
                    Insignificant
                );
                for adj in neighbors(neighbor) {
                    if maze.get_cell(&adj) == CellType::Free && adj != start {
                        send_log(
                            channel_sender,
                            player_name,
                            &format!("Frontière trouvée : {:?} (type: {:?})", neighbor, maze.get_cell(&neighbor)),
                            Insignificant
                        );
                        if neighbor.x % 2 == 0 && neighbor.y % 2 == 0 {
                            return Some(neighbor);
                        }
                        return Some(adj);
                    }
                }
            }
            // Si ce n'est pas une frontière, continuer l'exploration BFS
            if maze.get_cell(&neighbor) == CellType::Free && !visited.contains(&neighbor) {
                send_log(
                    channel_sender,
                    player_name,
                    &format!("Ajout du voisin en visité : {:?} (type: {:?})", neighbor, maze.get_cell(&neighbor)),
                    Insignificant
                );
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    send_log(channel_sender, player_name, "AUCUNE FRONTIERE TROUVEE, THREAD MAY PANIC.", Mandatory);
    None
}

fn compute_direction(player_orientation: Orientation, wanted_direction: Orientation) -> RelativeDirection {
    // si je suis orienté vers l'est et que je veux aller vers le nord, je dois tourner à gauche
    // si je suis orienté vers l'est et que je veux aller vers le sud, je dois tourner à droite
    // etc...
    match player_orientation {
        Orientation::North => {
            match wanted_direction {
                Orientation::North => RelativeDirection::Front,
                Orientation::East => RelativeDirection::Right,
                Orientation::South => RelativeDirection::Back,
                Orientation::West => RelativeDirection::Left,
            }
        }
        Orientation::East => {
            match wanted_direction {
                Orientation::North => RelativeDirection::Left,
                Orientation::East => RelativeDirection::Front,
                Orientation::South => RelativeDirection::Right,
                Orientation::West => RelativeDirection::Back,
            }
        }
        Orientation::South => {
            match wanted_direction {
                Orientation::North => RelativeDirection::Back,
                Orientation::East => RelativeDirection::Left,
                Orientation::South => RelativeDirection::Front,
                Orientation::West => RelativeDirection::Right,
            }
        }
        Orientation::West => {
            match wanted_direction {
                Orientation::North => RelativeDirection::Right,
                Orientation::East => RelativeDirection::Back,
                Orientation::South => RelativeDirection::Left,
                Orientation::West => RelativeDirection::Front,
            }
        }
    }
}

fn move_to_next_step(
    player: &mut Player,
    player_tag: &str,
    next_step: Coord,
    channel_sender: &Sender<String>,
    tcp_stream: &mut TcpStream,
    last_action: &mut Option<Action>,
    last_player_coords: &mut Coord
) {
    // ex : next_step = Coord { x: 24, y: 10 } et player.coords = Coord { x: 22, y: 10 }
    let dx = next_step.x - player.coords.x;
    let dy = next_step.y - player.coords.y;
    // dx = 2, dy = 0

    let mut direction = RelativeDirection::Front;
    if dx == 2 && dy == 0 {
        direction = compute_direction(player.orientation.clone(), Orientation::East);
        player.orientation = Orientation::East;
    } else if dx == -2 && dy == 0 {
        direction = compute_direction(player.orientation.clone(), Orientation::West);
        player.orientation = Orientation::West;
    } else if dx == 0 && dy == 2 {
        direction = compute_direction(player.orientation.clone(), Orientation::North);
        player.orientation = Orientation::North;
    } else if dx == 0 && dy == -2 {
        direction = compute_direction(player.orientation.clone(), Orientation::South);
        player.orientation = Orientation::South;
    } else {
        send_log(
            channel_sender,
            player_tag,
            "Coordonnée invalide pour le déplacement",
            Mandatory
        );
    };

    let action = Action::MoveTo(direction);
    let client_request = ClientRequest::Action(action.clone());
    let json_message = serde_json::to_vec(&client_request).unwrap();

    send_log(
        channel_sender,
        player_tag,
        &format!("\nDéplacement : {:?} (message : {:?})", direction.clone(), client_request),
        Medium
    );
    send_encoded_message(tcp_stream, json_message);

    *last_player_coords = player.coords.clone();
    *last_action = Some(action);
    player.coords = next_step;
    send_log(
        channel_sender,
        player_tag,
        &format!("Coordonnées mises à jour : {:?}", player.coords),
        Medium
    );
    // player.coords = Coord { x: 24, y: 10 }
}

fn find_path_to_goal(
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
        &format!("Début de la recherche du chemin : start={:?}, goal={:?}", start, goal),
        Minor
    );

    while let Some(current) = queue.pop_front() {
        // Si on atteint l'objectif, reconstruire le chemin
        if current == goal {
            let mut path = vec![current];
            while let Some(&parent) = parent_map.get(&path.last().unwrap()) {
                path.push(parent);
            }
            path.reverse(); // On retourne le chemin pour partir du start

            send_log(channel_sender, player_tag, &format!("Chemin trouvé : {:?}", path), Minor);
            if path.is_empty() {
                send_log(channel_sender, player_tag, "!! WARNING !! : Aucun chemin trouvé (avant filtrage).", Mandatory);
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
                Medium
            );
            if filtered_path.is_empty() {
                send_log(channel_sender, player_tag, "!! WARNING !! : Aucun chemin trouvé (après filtrage).", Mandatory);
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
                Insignificant
            );
        }
    }

    // Si aucun chemin n'est trouvé, retourner un chemin vide
    send_log(channel_sender, player_tag, "Aucun chemin trouvé.", Mandatory);
    Vec::new()
}

fn play_game(channel_sender: Sender<String>, player_name: String, tcp_stream: &mut TcpStream, shared_numbers: Arc<Mutex<Vec<u64>>>, thread_index: usize) {
    let debug = false;
    let playing = true;
    let mut player = Player {
        coords: Coord { x: 0, y: 0 },
        orientation: Orientation::North
    };
    let mut maze = Maze::new();
    let mut goal_cell : Option<Coord> = None;

    let mut instructions : Vec<Coord> = vec![]; // liste des instructions pour atteindre l'objectif

    let mut last_action : Option<Action> = None;
    let mut last_player_coords : Coord = player.coords;

    'gameloop: while playing {
        match receive_encoded_message(tcp_stream) {
            ServerResponse::RadarView(radar_view) => {
                send_log(&channel_sender, &*player_name, &format!("Vue radar reçue : {}", radar_view), Medium);
                let decoded = decode_chelou64(&radar_view, debug);
                let player_tag = format!("{}", player_name.clone());
                if let Ok(data) = decoded {
                    if let Ok((radar_text, grid)) = decode_radar_data(&data, debug) {
                        send_log(&channel_sender, &*player_tag, &radar_text, Minor);
                        for i in 0..7 {
                            for j in 0..7 {
                                send_log(&channel_sender, &*player_tag, &format!("Cellule ({}, {}) : {}\n", i, j, grid[i][j]), Insignificant);
                                let (global_x, global_y) = local_to_global(channel_sender.clone(), &player_tag, &player, j as i16 - 3, 3 - i as i16);
                                send_log(&channel_sender, &*player_tag, &format!("Coordonnées globales : ({};{})", global_x, global_y), Insignificant);

                                let cell_type = match grid[i][j] {
                                    0 => CellType::Unknown,
                                    1 => CellType::Free,
                                    2 => CellType::Wall,
                                    10 => CellType::Goal,
                                    _ => CellType::Unknown,
                                };

                                if cell_type == CellType::Goal {
                                    send_log(&channel_sender, &*player_tag, &format!("!!! Objectif trouvé en {:?} !!!", (global_x, global_y)), Mandatory);
                                    goal_cell = Some(Coord { x: global_x, y: global_y });
                                }

                                // send_log(&channel_sender, &*player_tag, &format!("\nCoordonnées globales set : {:?}, CellType : {:?}", Coord { x: player.coords.x + global_x, y: player.coords.y + global_y }, cell_type));
                                maze.set_cell(Coord { x: global_x, y: global_y }, cell_type);
                            }
                        }
                        maze.display_numbers(&channel_sender, &*player_tag, 3);
                    }
                }

                send_log(&channel_sender, &*player_tag, &format!("Coordonnées globales du joueur : {:?}", player.coords), Minor);
                maze.display(&channel_sender, &*player_tag, 20);
                // send_log(&channel_sender, &*player_tag, &format!("Maze actuel : {:?}", maze.cells));


                if let Some(goal_coord) = goal_cell {
                    if let Some(&last) = instructions.last() {
                        if last != goal_coord {
                            instructions.clear();
                        }
                    }
                    send_log(&channel_sender, &*player_tag, &format!("Objectif trouvé en {:?}, déplacement en cours...", goal_coord), Major);
                    instructions.extend(find_path_to_goal(player.coords, goal_coord, &mut maze, &channel_sender, &player_name));
                    move_to_next_step(&mut player, &player_name, instructions.remove(0), &channel_sender, tcp_stream, &mut last_action, &mut last_player_coords);
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
                move_to_next_step(&mut player, &player_name, instructions.remove(0), &channel_sender, tcp_stream, &mut last_action, &mut last_player_coords);
                send_log(&channel_sender, &*player_tag, "Dodo pendant 0,05 secondes...", Insignificant);
                thread::sleep(Duration::from_millis(50));
            },
            ServerResponse::Challenge(challenge) => {
                send_log(&channel_sender, &*player_name, &format!("Challenge reçu : {:?}", challenge), Major);
                match challenge {
                    common::Challenge::SecretSumModulo(number) => {
                        let mut shared_mutex = shared_numbers.lock().unwrap();
                        send_log(&channel_sender, &*player_name, &format!("Nombres partagés : {:?}", *shared_mutex), Minor);
                        let sum_mod = shared_mutex.iter().fold(0u64, |acc, &val| {
                            (acc + (val % number)) % number
                        });
                        send_log(&channel_sender, &*player_name, &format!("Somme des nombres partagés (avec modulo appliqué) : {}", sum_mod), Major);
                        let action = Action::SolveChallenge { answer: sum_mod.to_string() };
                        let json_message = serde_json::to_vec(&ClientRequest::Action(action.clone())).unwrap();
                        send_log(&channel_sender, &*player_name, &format!("Réponse au challenge : {:?}", action), Major);
                        send_encoded_message(tcp_stream, json_message);
                        // shared_numbers : set all 3 to 0
                        *shared_mutex = vec![0; 3];
                    }
                    common::Challenge::SOS => {
                        send_log(&channel_sender, &*player_name, "SOS reçu, mais je ne sais pas quoi faire...", Mandatory);
                    }
                }
            },
            ServerResponse::Hint(hint) => {
                send_log(&channel_sender, &*player_name, &format!("Indice reçu : {:?}", hint), Minor);
                match hint {
                    common::Hint::Secret(number) => {
                        send_log(&channel_sender, &*player_name, &format!("Indice secret reçu : {}", number), Mandatory);
                        shared_numbers.lock().unwrap()[thread_index] = number;
                        continue 'gameloop;
                    },
                    common::Hint::GridSize { columns, rows } => {
                        send_log(&channel_sender, &*player_name, &format!("Taille de la grille : {} colonnes, {} lignes", columns, rows), Mandatory);
                        continue 'gameloop;
                    },
                    common::Hint::RelativeCompass { angle } => {
                        send_log(&channel_sender, &*player_name, &format!("Indication de direction relative : angle = {}", angle), Mandatory);
                        continue 'gameloop;
                    }
                }
            },
            ServerResponse::ActionError(action_error) => {
                send_log(&channel_sender, &*player_name, &format!("Erreur d'action reçue : {:?}", action_error), Mandatory);
                match action_error {
                    common::ActionError::CannotPassThroughWall => {
                        send_log(&channel_sender, &*player_name, "Impossible de passer à travers un mur", Mandatory);
                        player.coords = last_player_coords;
                        send_log(&channel_sender, &*player_name, &format!("Cancel de l'action précédente : {:?}", last_action), Medium);
                        send_log(&channel_sender, &*player_name, &format!("Nouvelles coordonnées du joueur : {:?}", last_player_coords), Medium);
                    },
                    common::ActionError::NoRunningChallenge => {
                        send_log(&channel_sender, &*player_name, "Aucun challenge en cours", Mandatory);
                    },
                    common::ActionError::SolveChallengeFirst => {
                        send_log(&channel_sender, &*player_name, "Résoudre le challenge en cours avant de se déplacer", Mandatory);
                    },
                    common::ActionError::InvalidChallengeSolution => {
                        send_log(&channel_sender, &*player_name, "Solution de challenge invalide", Mandatory);
                        break 'gameloop;
                    },
                }
            },
            msg => {
                send_log(&channel_sender, &*player_name, &format!("Message inattendu reçu : {:?}", msg), Mandatory);
            }
        }
    }
}

fn distance(player: Coord, goal: Coord) -> u64 {
    ((player.x - goal.x).abs() + (player.y - goal.y).abs()) as u64
}

fn local_to_global(channel_sender: Sender<String>, player_tag: &str, player: &Player, x: i16, y: i16) -> (i16, i16) {
    // -2 et -1 NORTH = -2,-1
    // -2 et -1 EAST = -1,2
    // -2 et -1 SOUTH = 2,1
    // -2 et -1 WEST = 1,-2
    match player.orientation {
        Orientation::North => {
            // log : (0,0) + (-2,-1) = (-2,-1)
            send_log(&channel_sender, &*player_tag, &format!("log : {:?} + {:?} = {:?}", player.coords, (x, y), (player.coords.x + x, player.coords.y + y)), Insignificant);
            (player.coords.x + x, player.coords.y + y)
        },
        Orientation::East => {
            // log : (0,0) + (-2,-1) = (-1,2)
            send_log(&channel_sender, &*player_tag, &format!("log : {:?} + {:?} = {:?}", player.coords, (y, -x), (player.coords.x + y, player.coords.y - x)), Insignificant);
            (player.coords.x + y, player.coords.y - x)
        },
        Orientation::South => {
            // log : (0,0) + (-2,-1) = (2,1)
            send_log(&channel_sender, &*player_tag, &format!("log : {:?} + {:?} = {:?}", player.coords, (-x, -y), (player.coords.x - x, player.coords.y - y)), Insignificant);
            (player.coords.x - x, player.coords.y - y)
        },
        Orientation::West => {
            // log : (0,0) + (-2,-1) = (1,-2)
            send_log(&channel_sender, &*player_tag, &format!("log : {:?} + {:?} = {:?}", player.coords, (-y, x), (player.coords.x - y, player.coords.y + x)), Insignificant);
            (player.coords.x - y, player.coords.y + x)
        },
    }
}
