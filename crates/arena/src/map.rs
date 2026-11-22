//! La carte, lue dans un fichier texte (D-010).
//!
//! Une ligne par rangée `y`, un caractère par case `x` : `.` vide, `#` mur, `A` et `B` les départs des agents 0 et 1,
//! `1` à `9` une ressource de cette quantité. Toutes les lignes ont la même longueur.

/// Une grille et son contenu de départ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    width: i64,
    height: i64,
    /// Indexés `y * width + x`, comme `resources`.
    walls: Vec<bool>,
    resources: Vec<i64>,
    starts: [(i64, i64); 2],
}

/// Une carte mal formée. La ligne vaut 0 quand l'erreur porte sur la carte entière (vide, départ manquant).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ligne {line} : {message}")]
pub struct MapError {
    pub line: usize,
    pub message: String,
}

impl Map {
    /// Lit une carte. Les `\r` des fins de ligne Windows et un retour à la ligne final sont tolérés.
    pub fn parse(text: &str) -> Result<Map, MapError> {
        let mut width = None;
        let mut height = 0;
        let mut walls = Vec::new();
        let mut resources = Vec::new();
        let mut starts = [None, None];
        for (index, row) in text.lines().enumerate() {
            let line = index.saturating_add(1);
            let cells = row.chars().count();
            match width {
                None if cells == 0 => return error(line, "rangée vide"),
                None => width = Some(cells),
                Some(expected) if expected != cells => {
                    return error(line, format!("{cells} cases au lieu de {expected}"));
                }
                Some(_) => {}
            }
            for (x, c) in (0_i64..).zip(row.chars()) {
                let (wall, quantity) = match c {
                    '.' => (false, 0),
                    '#' => (true, 0),
                    'A' | 'B' => {
                        let slot = if c == 'A' {
                            &mut starts[0]
                        } else {
                            &mut starts[1]
                        };
                        if slot.replace((x, height)).is_some() {
                            return error(line, format!("deuxième départ {c}"));
                        }
                        (false, 0)
                    }
                    '1'..='9' => (false, i64::from(c.to_digit(10).unwrap_or(0))),
                    other => return error(line, format!("caractère inconnu « {other} »")),
                };
                walls.push(wall);
                resources.push(quantity);
            }
            height += 1;
        }
        let Some(width) = width else {
            return error(0, "carte vide");
        };
        let [Some(a), Some(b)] = starts else {
            return error(0, "il faut un départ A et un départ B");
        };
        Ok(Map {
            width: i64::try_from(width).unwrap_or(i64::MAX),
            height,
            walls,
            resources,
            starts: [a, b],
        })
    }

    pub fn width(&self) -> i64 {
        self.width
    }

    pub fn height(&self) -> i64 {
        self.height
    }

    pub fn in_bounds(&self, x: i64, y: i64) -> bool {
        (0..self.width).contains(&x) && (0..self.height).contains(&y)
    }

    /// `false` hors de la grille : le bord bloque `MOVE` (E.3), mais il n'est pas une case murée.
    pub fn is_wall(&self, x: i64, y: i64) -> bool {
        self.index(x, y)
            .and_then(|i| self.walls.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// `0` hors de la grille.
    pub fn resource(&self, x: i64, y: i64) -> i64 {
        self.index(x, y)
            .and_then(|i| self.resources.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// Les départs des agents 0 et 1.
    pub fn starts(&self) -> [(i64, i64); 2] {
        self.starts
    }

    /// Sans effet hors de la grille.
    pub(crate) fn set_resource(&mut self, x: i64, y: i64, quantity: i64) {
        if let Some(cell) = self.index(x, y).and_then(|i| self.resources.get_mut(i)) {
            *cell = quantity;
        }
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        if !self.in_bounds(x, y) {
            return None;
        }
        usize::try_from(y.checked_mul(self.width)?.checked_add(x)?).ok()
    }
}

fn error<T>(line: usize, message: impl Into<String>) -> Result<T, MapError> {
    Err(MapError {
        line,
        message: message.into(),
    })
}
