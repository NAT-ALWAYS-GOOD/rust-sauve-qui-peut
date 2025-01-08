use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use common::{ClientRequest, ServerResponse, RegisterTeamResult, SubscribePlayerResult, send_encoded_message, receive_encoded_message};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    NORTH,
    SOUTH,
    EAST,
    WEST
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Unknown, // Pas encore exploré
    Free,    // 1 : passage libre
    Wall,    // 2 : mur
    Goal,    // 10 : objectif
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}
struct Player {
    coords: Coord,
    orientation: Orientation
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
        self.cells.insert(coord, cell_type);
    }

    pub fn get_cell(&self, coord: &Coord) -> CellType {
        *self.cells.get(coord).unwrap_or(&CellType::Unknown)
    }

    pub fn display(&self, channel_sender: &Sender<String>, tag: &str, radius: i16) {
        let mut output = String::new();
        output.push_str("=== Grille 7x7 ===\n");

        for x in -radius..=radius {
            for y in (-radius..=radius).rev() {
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

        send_log(channel_sender, tag, &output);
    }
}


fn send_log(channel_sender: &Sender<String>, tag: &str, message: &str) {
    // FIXME: only send logs if tag is "Joueur 3"
    if tag == "Joueur 3" {
        channel_sender.send(format!("[{}] : {}", tag, message)).unwrap();
    }
    // channel_sender.send(format!("[{}] : {}", tag, message)).unwrap();
}

fn register_team(channel_sender: &Sender<String>, server_address: &str) -> Option<String> {
    println!("Tentative de connexion au serveur {} pour l'équipe...", server_address);
    let team_tag = "Équipe";
    match TcpStream::connect(server_address) {
        Ok(mut stream) => {
            send_log(channel_sender, team_tag, "Connecté au serveur pour l'équipe");

            let team_name = format!("NAT Team {}", rand::random::<u32>());
            let register_team = ClientRequest::RegisterTeam { name: team_name };

            send_log(channel_sender, team_tag, &format!("Envoi de l'enregistrement de l'équipe : {:?}", register_team));
            let json_message = serde_json::to_vec(&register_team).unwrap();
            send_encoded_message(&mut stream, json_message);

            send_log(channel_sender, team_tag, "Attente de la réponse du serveur pour l'équipe");
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
                    );
                    Some(registration_token)
                }
                ServerResponse::RegisterTeamResult(RegisterTeamResult::Err(error)) => {
                    send_log(channel_sender, team_tag, &format!("Erreur lors de l'enregistrement de l'équipe : {:?}", error));
                    None
                }
                _ => {
                    send_log(channel_sender, team_tag, "Message inattendu reçu lors de l'enregistrement");
                    None
                }
            }
        }
        Err(e) => {
            send_log(channel_sender, team_tag, &format!("Impossible de se connecter au serveur : {}", e));
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

    send_log(channel_sender, player_tag, &format!("Envoi de l'enregistrement du joueur : {:?}", subscribe_player));
    let json_message = serde_json::to_vec(&subscribe_player).unwrap();
    send_encoded_message(tcp_stream, json_message);

    send_log(channel_sender, player_tag, "Attente de la réponse du serveur");
    match receive_encoded_message(tcp_stream) {
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Ok) => {
            send_log(channel_sender, player_tag, "Enregistrement réussi");
            true
        }
        ServerResponse::SubscribePlayerResult(SubscribePlayerResult::Err(error)) => {
            send_log(channel_sender, player_tag, &format!("Erreur lors de l'enregistrement : {:?}", error));
            false
        }
        _ => {
            send_log(channel_sender, player_tag, "Message inattendu reçu lors de l'enregistrement");
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

    for player in players {
        let thread = launch_player_thread(
            server_address.parse().unwrap(),
            player.parse().unwrap(),
            channel_sender.clone(),
            reg_token.clone(),
        );
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

fn launch_player_thread(server_address: String, player: String, channel_sender_cloned: Sender<String>, reg_token_cloned: String) -> JoinHandle<()> {
    thread::spawn(move || {
        match TcpStream::connect(&server_address) {
            Ok(mut stream) => {
                send_log(&channel_sender_cloned, &*player, "Connecté au serveur");
                let registered = register_player(&channel_sender_cloned, &*player, &*reg_token_cloned, &mut stream);
                if !registered {
                    send_log(&channel_sender_cloned, &*player, "Erreur d'enregistrement, arrêt du thread.");
                    return;
                } else {
                    play_game(channel_sender_cloned, player, &mut stream);
                }
            }
            Err(e) => {
                send_log(&channel_sender_cloned, &*player, &format!("Impossible de se connecter au serveur : {}", e));
            }
        }
    })
}

fn local_to_global(
    player_x: i16,
    player_y: i16,
    orientation: Orientation,
    i: i16,
    j: i16
) -> (i16, i16) {
    let dx = i - 3;
    let dy = 3 - j;

    let (rot_x, rot_y) = match orientation {
        Orientation::NORTH => ( dx,      dy),
        Orientation::EAST  => ( dy,     -dx),
        Orientation::SOUTH => (-dx,     -dy),
        Orientation::WEST  => (-dy,      dx),
    };

    (player_x + rot_x, player_y + rot_y)
}

fn play_game(channel_sender: Sender<String>, player_name: String, tcp_stream: &mut TcpStream) {
    let debug = false;
    let playing = true;
    let mut player = Player {
        coords: Coord { x: 0, y: 0 },
        orientation: Orientation::NORTH,
    };
    let mut maze = Maze::new();
    while playing {
        match receive_encoded_message(tcp_stream) {
            ServerResponse::RadarView(radar_view) => {
                send_log(&channel_sender, &*player_name, &format!("Vue radar reçue : {}", radar_view));
                let decoded = decode_chelou64(&radar_view, debug);
                match decoded {
                    Ok(data) => {
                        match decode_radar_data(&data, debug) {
                            Ok((radar_text, grid)) => {
                                let player_tag = format!("{}", player_name.clone());
                                send_log(&channel_sender, &*player_tag, &radar_text);
                                for i in 0..7 {
                                    for j in 0..7 {
                                        let (global_x, global_y) = local_to_global(player.coords.x, player.coords.y, player.orientation, i, j);
                                        let cell_type = match grid[i as usize][j as usize] {
                                            0 => CellType::Unknown,
                                            1 => CellType::Free,
                                            2 => CellType::Wall,
                                            10 => CellType::Goal,
                                            _ => CellType::Unknown
                                        };
                                        maze.set_cell(Coord { x: global_x, y: global_y }, cell_type);
                                    }
                                }
                                send_log(&channel_sender, &*player_tag, &format!("Maze actuel : {:?}", maze.cells));
                                maze.display(&channel_sender, &*player_tag, 10);
                                send_log(&channel_sender, &*player_tag, &format!("Coordonnées globales du joueur : {:?}", player.coords));
                                send_log(&channel_sender, &*player_tag, &format!("Orientation du joueur : {:?}", player.orientation));
                                send_log(&channel_sender, &*player_tag, "Dodo pendant 10 secondes...");
                                thread::sleep(Duration::from_secs(10));
                                // Effectuer une action en fonction
                            }
                            Err(err) => {
                                eprintln!("Erreur dans decode_radar_data : {}", err);
                            }
                        }
                    }
                    Err(err) => {
                        send_log(
                            &channel_sender,
                            &*player_name,
                            &format!("Erreur lors du décodage b64 : {}", err),
                        );
                    }
                }
            }
            msg => {
                send_log(&channel_sender, &*player_name, &format!("Message inattendu reçu : {:?}", msg));
            }
        }
    }
}