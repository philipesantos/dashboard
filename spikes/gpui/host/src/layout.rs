//! The dashboard grid: where each widget sits, in cells, and the rules for moving and resizing.

use serde::{Deserialize, Serialize};

pub const COLUMNS: i32 = 12;
pub const MIN_W: i32 = 2;
pub const MIN_H: i32 = 2;
pub const MAX_H: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Cell {
    fn overlaps(&self, other: &Cell) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    fn fits(&self) -> bool {
        self.x >= 0
            && self.y >= 0
            && self.w >= MIN_W
            && self.h >= MIN_H
            && self.h <= MAX_H
            && self.x + self.w <= COLUMNS
    }
}

/// One widget on the grid: which catalog entry it shows and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub id: u64,
    pub entry: String,
    pub cell: Cell,
    /// Host-held state the host passes in the widget's configuration (the sampler's count).
    #[serde(default)]
    pub clicks: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub widgets: Vec<Placed>,
    pub next_id: u64,
}

impl Layout {
    /// Whether `cell` is inside the grid and free of every widget but `except`.
    pub fn is_free(&self, cell: &Cell, except: Option<u64>) -> bool {
        cell.fits()
            && self
                .widgets
                .iter()
                .filter(|w| Some(w.id) != except)
                .all(|w| !w.cell.overlaps(cell))
    }

    /// Adds a widget of `w`×`h` cells at the first free place, top to bottom, left to right.
    pub fn add(&mut self, entry: &str, w: i32, h: i32) -> u64 {
        let mut y = 0;
        let cell = loop {
            if let Some(cell) = (0..=COLUMNS - w)
                .map(|x| Cell { x, y, w, h })
                .find(|cell| self.is_free(cell, None))
            {
                break cell;
            }
            y += 1;
        };
        let id = self.next_id;
        self.next_id += 1;
        self.widgets.push(Placed {
            id,
            entry: entry.to_owned(),
            cell,
            clicks: 0,
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<&Placed> {
        self.widgets.iter().find(|w| w.id == id)
    }

    /// Puts widget `id` at `cell` if that place is free; returns whether it moved.
    pub fn place(&mut self, id: u64, cell: Cell) -> bool {
        if !self.is_free(&cell, Some(id)) {
            return false;
        }
        match self.widgets.iter_mut().find(|w| w.id == id) {
            Some(widget) => {
                widget.cell = cell;
                true
            }
            None => false,
        }
    }

    /// Moves by whole cells; a blocked move does nothing.
    pub fn nudge(&mut self, id: u64, dx: i32, dy: i32) -> bool {
        let Some(cell) = self.get(id).map(|w| w.cell) else {
            return false;
        };
        self.place(
            id,
            Cell {
                x: cell.x + dx,
                y: cell.y + dy,
                ..cell
            },
        )
    }

    /// Resizes by whole cells; a blocked resize does nothing.
    pub fn grow(&mut self, id: u64, dw: i32, dh: i32) -> bool {
        let Some(cell) = self.get(id).map(|w| w.cell) else {
            return false;
        };
        self.place(
            id,
            Cell {
                w: cell.w + dw,
                h: cell.h + dh,
                ..cell
            },
        )
    }

    pub fn remove(&mut self, id: u64) {
        self.widgets.retain(|w| w.id != id);
    }

    /// The number of rows the grid shows: every widget plus room to drop below them.
    pub fn rows(&self) -> i32 {
        self.widgets
            .iter()
            .map(|w| w.cell.y + w.cell.h)
            .max()
            .unwrap_or(0)
            + 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widgets_never_overlap() {
        let mut layout = Layout::default();
        let a = layout.add("a", 4, 3);
        let b = layout.add("b", 4, 3);
        assert_eq!(layout.get(b).unwrap().cell.x, 4);
        // b cannot move onto a, nor grow into it
        assert!(!layout.nudge(b, -1, 0));
        assert!(!layout.place(
            b,
            Cell {
                x: 2,
                y: 1,
                w: 4,
                h: 3
            }
        ));
        assert!(layout.nudge(b, 1, 0));
        assert!(!layout.grow(a, 2, 0));
        assert!(layout.grow(a, 1, 0));
    }

    #[test]
    fn widgets_stay_on_the_grid() {
        let mut layout = Layout::default();
        let a = layout.add("a", 4, 3);
        assert!(!layout.nudge(a, -1, 0));
        assert!(!layout.nudge(a, 0, -1));
        assert!(!layout.place(
            a,
            Cell {
                x: 10,
                y: 0,
                w: 4,
                h: 3
            }
        ));
        assert!(!layout.grow(a, -3, 0));
        assert!(layout.place(
            a,
            Cell {
                x: 8,
                y: 0,
                w: 4,
                h: 3
            }
        ));
    }

    #[test]
    fn a_saved_layout_reads_back() {
        let mut layout = Layout::default();
        layout.add("a", 4, 3);
        layout.add("b", 6, 4);
        let text = serde_json::to_string(&layout).unwrap();
        assert_eq!(serde_json::from_str::<Layout>(&text).unwrap(), layout);
    }
}
