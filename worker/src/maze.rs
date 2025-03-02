//! Module gérant les labyrinthes.

use crate::logger::send_log;
use crate::logger::LogType::Minor;
use std::collections::HashMap;
use std::sync::mpsc::Sender;

/// Représente le type d'une cellule dans le labyrinthe.
///
/// - `Unknown` : La cellule n'a pas encore été explorée.
/// - `Free`    : Passage libre.
/// - `Wall`    : Mur.
/// - `Goal`    : Objectif à atteindre.
/// - `Ally`    : Cellule occupée par un allié.
/// - `Enemy`   : Cellule occupée par un ennemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Unknown, // Pas encore exploré
    Free,    // 1 : passage libre
    Wall,    // 2 : mur
    Goal,    // 10 : objectif
    Ally,
    Enemy,
}

/// Représente l'orientation d'un joueur ou d'un élément dans le labyrinthe.
///
/// Permet d'indiquer la direction vers le nord, l'est, le sud ou l'ouest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    North,
    East,
    South,
    West,
}

/// Représente une coordonnée 2D dans le labyrinthe.
///
/// Les coordonnées sont composées de deux valeurs entières (`x` et `y`).
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

/// Représente un joueur dans le labyrinthe.
///
/// Contient la position actuelle (`coords`) et l'orientation (`orientation`) du joueur.
pub struct Player {
    pub(crate) coords: Coord,
    pub(crate) orientation: Orientation,
}

/// Représente le labyrinthe sous forme d'une table de cellules.
///
/// La structure utilise une `HashMap` pour associer à chaque coordonnée un type de cellule.
pub struct Maze {
    pub cells: HashMap<Coord, CellType>,
}

impl Maze {
    /// Crée un nouveau labyrinthe vide.
    ///
    /// # Exemple
    ///
    /// ```rust
    /// let maze = Maze::new();
    /// ```
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    /// Définit le type d'une cellule à une coordonnée donnée.
    ///
    /// Si la cellule à cette coordonnée est déjà connue et différente de `Unknown`,
    /// la fonction ne modifie pas sa valeur.
    ///
    /// # Arguments
    ///
    /// * `coord` - La coordonnée de la cellule à mettre à jour.
    /// * `cell_type` - Le type de cellule à assigner.
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

    /// Retourne le type de cellule à une coordonnée donnée.
    ///
    /// Si la cellule n'existe pas dans la map, la fonction retourne `CellType::Unknown`.
    ///
    /// # Arguments
    ///
    /// * `coord` - La référence de la coordonnée à consulter.
    ///
    /// # Retour
    ///
    /// Le type de cellule présent à la coordonnée.
    pub fn get_cell(&self, coord: &Coord) -> CellType {
        let cell_type: Option<&CellType> = self.cells.get(coord);
        match cell_type {
            Some(cell) => *cell,
            None => CellType::Unknown,
        }
    }

    /// Affiche une représentation numérique du labyrinthe dans un format 7x7.
    ///
    /// Pour chaque cellule, un nombre est affiché :
    /// - `0` pour `Unknown`
    /// - `1` pour `Free`
    /// - `2` pour `Wall`
    /// - `5` pour `Ally`
    /// - `6` pour `Enemy`
    /// - `10` pour `Goal`
    ///
    /// Le résultat est envoyé via le canal de log fourni.
    ///
    /// # Arguments
    ///
    /// * `channel_sender` - Le canal utilisé pour envoyer le log.
    /// * `tag` - L'étiquette à utiliser dans le log.
    /// * `radius` - Le rayon autour du centre (0,0) à afficher (par exemple, pour 7x7, le rayon est 3).
    pub fn display_numbers(&self, channel_sender: &Sender<String>, tag: &str, radius: i16) {
        let mut output = String::new();
        output.push_str("=== Maze Nombres 7x7 ===\n");

        for y in (-radius..=radius).rev() {
            for x in -radius..=radius {
                let coord = Coord { x, y };
                let cell = self.get_cell(&coord);
                let cell_str = match cell {
                    CellType::Unknown => "0",
                    CellType::Free => "1",
                    CellType::Wall => "2",
                    CellType::Ally => "5",
                    CellType::Enemy => "6",
                    CellType::Goal => "10",
                };
                output.push_str(cell_str);
            }
            output.push_str("\n");
        }

        send_log(channel_sender, tag, &output, Minor);
    }

    /// Affiche une représentation graphique du labyrinthe dans un format 7x7.
    ///
    /// Utilise des emojis pour représenter chaque type de cellule :
    /// - `"🟫"` pour `Unknown`
    /// - `"⬜️"` pour `Free`
    /// - `"⬛️"` pour `Wall`
    /// - `"👥"` pour `Ally`
    /// - `"🚷"` pour `Enemy`
    /// - `"🎯"` pour `Goal`
    ///
    /// La position du joueur est indiquée par une flèche correspondant à son orientation, et
    /// le centre (0,0) est marqué par `"0️⃣"`.
    ///
    /// Le résultat est envoyé via le canal de log fourni.
    ///
    /// # Arguments
    ///
    /// * `channel_sender` - Le canal utilisé pour envoyer le log.
    /// * `tag` - L'étiquette à utiliser dans le log.
    /// * `radius` - Le rayon autour du centre à afficher.
    /// * `player` - La référence vers le joueur afin d'indiquer sa position et son orientation.
    pub fn display(
        &self,
        channel_sender: &Sender<String>,
        tag: &str,
        radius: i16,
        player: &Player,
    ) {
        let mut output = String::new();
        output.push_str("=== Maze 7x7 ===\n");

        for y in (-radius..=radius).rev() {
            for x in -radius..=radius {
                let coord = Coord { x, y };
                let cell = self.get_cell(&coord);
                if coord.x == 0 && coord.y == 0 {
                    output.push_str("0️⃣");
                    continue;
                }
                if coord == player.coords {
                    let orientation_str = match player.orientation {
                        Orientation::North => "🔼",
                        Orientation::East => "▶️",
                        Orientation::South => "🔽",
                        Orientation::West => "◀️",
                    };
                    output.push_str(orientation_str);
                    continue;
                }
                let cell_str = match cell {
                    CellType::Unknown => "🟫",
                    CellType::Free => "⬜️",
                    CellType::Ally => "👥",
                    CellType::Enemy => "🚷",
                    CellType::Wall => "⬛️",
                    CellType::Goal => "🎯",
                };
                output.push_str(cell_str);
            }
            output.push_str("\n");
        }

        send_log(channel_sender, tag, &output, Minor);
    }
}

/// Retourne les cellules voisines (adjacentes) à une coordonnée donnée.
///
/// Les voisins sont calculés dans les 4 directions : haut, bas, gauche et droite.
///
/// # Arguments
///
/// * `coord` - La coordonnée dont on souhaite obtenir les voisins.
///
/// # Retour
///
/// Un vecteur contenant les coordonnées des cellules voisines.
///
/// # Exemple
///
/// ```rust
/// let coord = Coord { x: 0, y: 0 };
/// let voisins = neighbors(coord);
/// assert_eq!(voisins, vec![
///     Coord { x: 0, y: 1 },
///     Coord { x: 0, y: -1 },
///     Coord { x: -1, y: 0 },
///     Coord { x: 1, y: 0 },
/// ]);
/// ```
pub fn neighbors(coord: Coord) -> Vec<Coord> {
    vec![
        Coord {
            x: coord.x,
            y: coord.y + 1,
        },
        Coord {
            x: coord.x,
            y: coord.y - 1,
        },
        Coord {
            x: coord.x - 1,
            y: coord.y,
        },
        Coord {
            x: coord.x + 1,
            y: coord.y,
        },
    ]
}

#[cfg(test)]
mod maze_tests {
    use super::*;

    #[test]
    fn test_neighbors() {
        let coord = Coord { x: 0, y: 0 };
        let expected = vec![
            Coord { x: 0, y: 1 },
            Coord { x: 0, y: -1 },
            Coord { x: -1, y: 0 },
            Coord { x: 1, y: 0 },
        ];
        assert_eq!(neighbors(coord), expected);
    }
}
