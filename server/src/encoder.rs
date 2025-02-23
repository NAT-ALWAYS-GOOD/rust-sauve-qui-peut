pub fn encode_chelou64(input: &[u8]) -> String {
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/";
    let mut output = String::new();
    let mut i = 0;

    while i < input.len() {
        let remaining = input.len() - i;
        let block_size = remaining.min(3);

        let mut chunk = [0u8; 3];
        for j in 0..block_size {
            chunk[j] = input[i + j];
        }

        // Encodage des 3 octets en 4 caractères de 6 bits
        let first_char = (chunk[0] >> 2) & 0x3F;
        output.push(charset.chars().nth(first_char as usize).unwrap());

        let second_char = ((chunk[0] & 0x03) << 4) | ((chunk[1] >> 4) & 0x0F);
        output.push(charset.chars().nth(second_char as usize).unwrap());

        if block_size > 1 {
            let third_char = ((chunk[1] & 0x0F) << 2) | ((chunk[2] >> 6) & 0x03);
            output.push(charset.chars().nth(third_char as usize).unwrap());
        }

        if block_size > 2 {
            let fourth_char = chunk[2] & 0x3F;
            output.push(charset.chars().nth(fourth_char as usize).unwrap());
        }

        i += block_size;
    }

    output
}

pub fn encode_radar_data(internal_grid: &[Vec<u8>]) -> Vec<u8> {
    if internal_grid.len() != 7 || internal_grid.iter().any(|row| row.len() != 7) {
        panic!("Grille invalide : la grille doit être 7x7");
    }

    // Coordonnées dans la grille interne
    let hcoords = [
        (0, 1), (0, 3), (0, 5),
        (2, 1), (2, 3), (2, 5),
        (4, 1), (4, 3), (4, 5),
        (6, 1), (6, 3), (6, 5),
    ];
    let vcoords = [
        (1, 0), (1, 2), (1, 4), (1, 6),
        (3, 0), (3, 2), (3, 4), (3, 6),
        (5, 0), (5, 2), (5, 4), (5, 6),
    ];
    let ccoords = [
        (1, 1), (1, 3), (1, 5),
        (3, 1), (3, 3), (3, 5),
        (5, 1), (5, 3), (5, 5),
    ];

    let mut horiz_bits: u32 = 0;
    for (i, &(r, c)) in hcoords.iter().enumerate() {
        let passage = internal_grid[r][c] & 0b11;
        let shift = 24 - 2 * ((i as u32) + 1);
        horiz_bits |= (passage as u32) << shift;
    }
    let horiz_bytes = &horiz_bits.to_le_bytes()[0..3];

    let mut vert_bits: u32 = 0;
    for (i, &(r, c)) in vcoords.iter().enumerate() {
        let passage = internal_grid[r][c] & 0b11;
        let shift = 24 - 2 * ((i as u32) + 1);
        vert_bits |= (passage as u32) << shift;
    }
    let vert_bytes = &vert_bits.to_le_bytes()[0..3];

    let mut cell_nibbles = Vec::with_capacity(10);
    for &(r, c) in ccoords.iter() {
        let cell_val = internal_grid[r][c];
        let nibble = match cell_val {
            0 => 0b1111,
            1 => 0b0000,
            5 => 0b0001,
            6 => 0b0010,
            10 => 0b1000,
            _ => 0,
        };
        cell_nibbles.push(nibble);
    }
    // 10ème nibble
    cell_nibbles.push(0);

    // 5 octets pour les cellules
    let mut cell_bytes = Vec::with_capacity(5);
    for i in 0..5 {
        let high = cell_nibbles[2 * i];
        let low = cell_nibbles[2 * i + 1];
        cell_bytes.push((high << 4) | (low & 0x0F));
    }

    let mut output = Vec::with_capacity(11);
    output.extend_from_slice(horiz_bytes);
    output.extend_from_slice(vert_bytes);
    output.extend_from_slice(&cell_bytes);

    output
}


#[cfg(test)]
mod encoder_tests {
    use super::*;

    #[test]
    fn should_encode_chelou64() {
        let input = vec![32, 70, 18, 128, 152, 40, 240, 240, 15, 15, 240];
        let result = encode_chelou64(&input);
        let expected_output = "ieysGjGO8papd/a".to_string();
        assert_eq!(result, expected_output);
    }

    fn get_internal_grid() -> Vec<Vec<u8>> {
        vec![
            vec![2, 0, 2, 1, 2, 0, 2],
            vec![0, 0, 2, 1, 2, 0, 0],
            vec![2, 2, 2, 1, 2, 0, 2],
            vec![2, 1, 1, 1, 2, 0, 0],
            vec![2, 1, 2, 2, 2, 0, 2],
            vec![2, 1, 0, 0, 0, 0, 0],
            vec![2, 2, 2, 0, 2, 0, 2]
        ]
    }

    #[test]
    fn should_encode_radar_data() {
        let internal_grid = get_internal_grid();
        let result = encode_radar_data(&internal_grid);
        let expected_output = vec![32, 70, 18, 128, 152, 40, 240, 240, 15, 15, 240];
        assert_eq!(result, expected_output);
    }

    #[test]
    fn should_encode_radar_data_with_goal() {
        let internal_grid = vec![
            vec![2, 0, 2, 1, 2, 0, 2],
            vec![0, 0, 2, 1, 2, 0, 0],
            vec![2, 2, 2, 1, 2, 0, 2],
            vec![2, 1, 1, 1, 2, 0, 0],
            vec![2, 1, 2, 2, 2, 0, 2],
            vec![2, 10, 0, 0, 0, 0, 0],
            vec![2, 2, 2, 0, 2, 0, 2]
        ];
        let result = encode_radar_data(&internal_grid);
        let expected_output = vec![32, 70, 18, 128, 152, 40, 240, 240, 15, 143, 240];
        assert_eq!(result, expected_output);
    }

    #[test]
    fn should_encode_all() {
        let internal_grid = get_internal_grid();
        let result = encode_radar_data(&internal_grid);
        let result_chelou64 = encode_chelou64(&result);
        let expected_output = "ieysGjGO8papd/a".to_string();
        assert_eq!(result_chelou64, expected_output);
    }
}