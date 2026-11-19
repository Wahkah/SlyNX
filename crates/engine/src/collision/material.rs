//! Friction and elasticity of contacts between materials (collision.md 6).

use formats::collision::{MATERIAL_COUNT, MaterialEntry, MaterialTables};

/// The two material tables, read from the player's executable (`formats::collision::MaterialTables`).
#[derive(Clone, Debug, PartialEq)]
pub struct Materials {
    tables: MaterialTables,
}

fn combine(table: &[MaterialEntry; MATERIAL_COUNT], i: i16, j: i16) -> f32 {
    let get = |m: i16| table[usize::try_from(m).ok().filter(|&m| m < MATERIAL_COUNT).unwrap_or(0)];
    let (a, b) = (get(i), get(j));
    let w = a.weight + b.weight;
    if w == 0.0 { 0.0 } else { (a.value + b.value) / w }
}

impl Materials {
    /// From the tables.
    pub fn new(tables: MaterialTables) -> Materials {
        Materials { tables }
    }

    /// Friction between materials `i` and `j`: `(a_i + a_j) / (w_i + w_j)`, at least 0. A body with no material
    /// uses 0; values outside the table count as 0.
    pub fn friction(&self, i: i16, j: i16) -> f32 {
        combine(&self.tables.friction, i, j).max(0.0)
    }

    /// Elasticity between materials `i` and `j`: the same weighted mean, clamped to `0..=1`.
    pub fn elasticity(&self, i: i16, j: i16) -> f32 {
        combine(&self.tables.elasticity, i, j).clamp(0.0, 1.0)
    }

    /// The tables.
    pub fn tables(&self) -> &MaterialTables {
        &self.tables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_means() {
        let mut f = [MaterialEntry { weight: 1.0, value: 0.5 }; MATERIAL_COUNT];
        let mut e = f;
        f[1] = MaterialEntry { weight: 3.0, value: 2.4 };
        f[12] = MaterialEntry { weight: 1.0, value: -10.0 };
        e[12] = MaterialEntry { weight: 1.0, value: 1.0 };
        let m = Materials::new(MaterialTables { friction: f, elasticity: e });
        assert_eq!(m.friction(0, 0), 0.5);
        assert!((m.friction(0, 1) - 0.725).abs() < 1e-6);
        assert_eq!(m.friction(12, 1), 0.0);
        assert_eq!(m.elasticity(12, 0), 0.75);
        assert_eq!(m.friction(-3, 99), 0.5);
    }
}
