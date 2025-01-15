use crate::logger::send_log;
use crate::logger::LogType::Minor;
use std::collections::HashMap;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Unknown, // Pas encore exploré
    Free,    // 1 : passage libre
    Wall,    // 2 : mur
    Goal,    // 10 : objectif
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
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
pub struct Player {
    pub(crate) coords: Coord,
    pub(crate) orientation: Orientation,
}

pub struct Maze {
    pub cells: HashMap<Coord, CellType>,
}

impl Maze {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new()
        }
    }

    pub fn set_cell(&mut self, coord: Coord, cell_type: CellType) {
        // Une cellule ne peut pas changer de type si elle est déjà connue et n'est pas de type Unknown
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

pub fn neighbors(coord: Coord) -> Vec<Coord> {
    vec![
        Coord { x: coord.x,     y: coord.y + 1 },
        Coord { x: coord.x,     y: coord.y - 1 },
        Coord { x: coord.x - 1, y: coord.y },
        Coord { x: coord.x + 1, y: coord.y }
    ]
}

pub fn distance(player: Coord, goal: Coord) -> u64 {
    ((player.x - goal.x).abs() + (player.y - goal.y).abs()) as u64
}