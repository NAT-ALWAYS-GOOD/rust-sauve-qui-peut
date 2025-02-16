pub fn decode_chelou64(input: &str, _debug: bool) -> Result<Vec<u8>, String> {
    let len = input.len();

    if len % 4 == 1 {
        return Err("Taille invalide (4n+1) pour cet encodage 6 bits/char".to_string());
    }

    fn decode_6bits_char(c: char) -> Option<u8> {
        match c {
            'a'..='z' => Some(c as u8 - b'a'),        // 0..25
            'A'..='Z' => Some(26 + (c as u8 - b'A')), // 26..51
            '0'..='9' => Some(52 + (c as u8 - b'0')), // 52..61
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

pub fn push_str_debug(output: &mut String, debug: bool, message: &str) {
    if debug {
        output.push_str(message);
    }
}

pub fn decode_radar_data(decoded: &[u8], debug: bool) -> Result<(String, Vec<Vec<u8>>), String> {
    if decoded.len() != 11 {
        return Err(format!(
            "Données incorrectes : {} octets reçus au lieu de 11",
            decoded.len()
        ));
    }

    let mut output = String::new();

    // Affichage des données brutes
    push_str_debug(
        &mut output,
        debug,
        "=== Données brutes (6 premiers octets) ===\n",
    );
    for i in 0..6 {
        push_str_debug(&mut output, debug, &format!("{:08b} ", decoded[i]));
    }
    push_str_debug(
        &mut output,
        debug,
        "\n=== Données brutes (5 derniers octets) ===\n",
    );
    for i in 6..11 {
        push_str_debug(&mut output, debug, &format!("{:02X} ", decoded[i]));
    }
    push_str_debug(&mut output, debug, "\n");

    let horiz_be = [decoded[0], decoded[1], decoded[2], 0];
    let horiz_bits = u32::from_le_bytes(horiz_be);
    push_str_debug(
        &mut output,
        debug,
        &format!("Bits horizontaux (little-endian) : {:032b}\n", horiz_bits),
    );

    let mut horizontal_passages = Vec::with_capacity(12);
    for i in 0..12 {
        let shift = 24 - 2 * (i + 1); // Calculer la position des 2 bits à extraire
        let two_bits = (horiz_bits >> shift) & 0b11; // Extraire les 2 bits
        push_str_debug(
            &mut output,
            debug,
            &format!("\nPassage '-' {} : {:b}", i + 1, two_bits as u8),
        );
        horizontal_passages.push(two_bits as u8);
    }

    let vert_be = [decoded[3], decoded[4], decoded[5], 0];
    let vert_bits = u32::from_le_bytes(vert_be);
    push_str_debug(
        &mut output,
        debug,
        &format!("Bits verticaux (little-endian) : {:032b}\n", vert_bits),
    );

    let mut vertical_passages = Vec::with_capacity(12);
    for i in 0..12 {
        let shift = 24 - 2 * (i + 1);
        let two_bits = (vert_bits >> shift) & 0b11;
        push_str_debug(
            &mut output,
            debug,
            &format!("\nPassage '|' {} : {:b}", i + 1, two_bits as u8),
        );
        vertical_passages.push(two_bits as u8);
    }

    let cell_bytes = [decoded[6], decoded[7], decoded[8], decoded[9], decoded[10]];

    let cells = handle_cells(&mut output, cell_bytes, debug);

    let mut grid = vec![vec!["•".to_string(); 7]; 7];
    let mut internal_grid: Vec<Vec<u8>> = vec![vec![2; 7]; 7];

    // Passages horizontaux
    let hcoords = [
        (0, 1),
        (0, 3),
        (0, 5),
        (2, 1),
        (2, 3),
        (2, 5),
        (4, 1),
        (4, 3),
        (4, 5),
        (6, 1),
        (6, 3),
        (6, 5),
    ];

    // Passages verticaux
    let vcoords = [
        (1, 0),
        (1, 2),
        (1, 4),
        (1, 6),
        (3, 0),
        (3, 2),
        (3, 4),
        (3, 6),
        (5, 0),
        (5, 2),
        (5, 4),
        (5, 6),
    ];

    // Coordonnées cellules (3×3)
    let ccoords = [
        (1, 1),
        (1, 3),
        (1, 5),
        (3, 1),
        (3, 3),
        (3, 5),
        (5, 1),
        (5, 3),
        (5, 5),
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
            0b0001 => 5,
            0b0101 => 5,
            0b0010 => 6,
            0b0110 => 6,
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
        let row_str = internal_grid[row]
            .iter()
            .map(|&val| format!("{}", val))
            .collect::<Vec<_>>()
            .join("");
        output.push_str(&format!("{}\n", row_str));
    }

    Ok((output, internal_grid))
}

pub fn handle_cells(output: &mut String, cell_bytes: [u8; 5], debug: bool) -> Vec<u8> {
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
        push_str_debug(
            output,
            debug,
            &format!("Cellule {} => nibble=0x{:X}\n", i + 1, val),
        );
    }

    // Petit affichage pour debug sur les cellules
    push_str_debug(output, debug, "\n=== Cellules (3x3)(remplissage) ===\n");
    for (i, &val) in cells.iter().enumerate() {
        let cell_info = decode_cell_info(val);
        if cell_info != "Rien" && cell_info != "Undefined" {
            output.push_str("\n\nWARNING : CELLULE NON VIDE REPEREE\n\n");
        }
        push_str_debug(
            output,
            debug,
            &format!(
                "Cellule {}: b-{:04b} h-{:X} => \"{}\"\n",
                i + 1,
                val,
                val,
                cell_info
            ),
        );
    }
    push_str_debug(output, debug, "\n");
    cells.to_vec()
}

pub fn decode_cell_symbol(nibble: u8) -> String {
    if nibble == 0b1111 {
        return "#".to_string();
    }
    let nature = (nibble & 0b1100) >> 2; // bits 3..2
    let entite = nibble & 0b0011; // bits 1..0

    let nature_char = match nature {
        0b00 => "",  // Rien
        0b01 => "H", // Indice
        0b10 => "G", // Objectif
        0b11 => "?", // Inconnu
        _ => unreachable!(),
    };
    let entite_char = match entite {
        0b00 => "",  // Aucune entité
        0b01 => "P", // Allié
        0b10 => "O", // Ennemi
        0b11 => "M", // Monstre
        _ => unreachable!(),
    };

    match (nature_char.is_empty(), entite_char.is_empty()) {
        (true, true) => " ".to_string(),
        (false, true) => nature_char.into(),
        (true, false) => entite_char.into(),
        (false, false) => format!("{}/{}", nature_char, entite_char),
    }
}

pub fn decode_cell_info(nibble: u8) -> String {
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

pub fn decode_nature_et_entite(nibble: u8) -> (String, String) {
    let nature = (nibble & 0b1100) >> 2;
    let entite = nibble & 0b0011;

    let nature_str = match nature {
        0b00 => "",
        0b01 => "Indice",
        0b10 => "Objectif",
        0b11 => "???", // non précisé => on met ???
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

#[cfg(test)]
mod decoder_tests {
    use super::*;

    #[test]
    fn should_decode_chelou64() {
        let input = "ieysGjGO8papd/a".to_string();
        let result = decode_chelou64(&input, false);
        // we expect binary to be :
        // 00100000 01000110 00010010 10000000 10011000 00101000 F0 F0 0F 0F F0
        // so in decimal : 32 70 18 128 152 40 240 240 15 15 240
        let vec1 = result.unwrap();
        println!("{:?}", vec1);
        assert_eq!(vec1, vec![32, 70, 18, 128, 152, 40, 240, 240, 15, 15, 240]);
    }

    #[test]
    fn should_decode_radar_data() {
        let input = vec![32, 70, 18, 128, 152, 40, 240, 240, 15, 15, 240];
        let result = decode_radar_data(&input, false);
        let (output, internal_grid) = result.unwrap();
        println!("{}", output);
        // we expect the grid to be :
        //         ##• •##
        //         ##| |##
        //         •-• •##
        //         |   |##
        //         • •-•##
        //         | #####
        //         •-•####
        // so :
        assert_eq!(
            internal_grid,
            vec![
                vec![2, 0, 2, 1, 2, 0, 2],
                vec![0, 0, 2, 1, 2, 0, 0],
                vec![2, 2, 2, 1, 2, 0, 2],
                vec![2, 1, 1, 1, 2, 0, 0],
                vec![2, 1, 2, 2, 2, 0, 2],
                vec![2, 1, 0, 0, 0, 0, 0],
                vec![2, 2, 2, 0, 2, 0, 2]
            ]
        );
    }
}
