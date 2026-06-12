//! Grid layout engine for Cloud Monitoring dashboard widget placement.
//!
//! Arranges widgets in a 12-column mosaic grid using a builder-pattern API.
//! The primary output is a flat `Vec<Tile>` ready for YAML serialization.

use crate::lib::dashboard::widgets::Widget;

/// A positioned widget on the 12-column mosaic grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub widget: Widget,
}

/// Internal representation of an entry within a row.
#[derive(Debug, Clone)]
enum RowEntry {
    /// A widget spanning a fixed number of columns.
    Span { cols: usize, widget: Widget },
    /// A sub-grid of widgets arranged N-per-row with automatic row wrapping.
    Grid { widgets: Vec<Widget>, cols_per_row: usize },
}

/// A horizontal band of widgets at a fixed height.
#[derive(Debug, Clone)]
pub struct Row {
    height: usize,
    entries: Vec<RowEntry>,
}

impl Row {
    /// Create a new row with the given height (in grid units).
    pub fn new(height: usize) -> Self {
        Self { height, entries: Vec::new() }
    }

    /// Add a full-width (12-column) widget.
    pub fn full(self, widget: Widget) -> Self {
        self.span(12, widget)
    }

    /// Add a half-width (6-column) widget.
    pub fn half(self, widget: Widget) -> Self {
        self.span(6, widget)
    }

    /// Add a widget spanning `cols` columns.
    pub fn span(mut self, cols: usize, widget: Widget) -> Self {
        self.entries.push(RowEntry::Span { cols, widget });
        self
    }

    /// Add a sub-grid that arranges widgets N-per-row with automatic wrapping.
    /// Cell width = 12 / cols_per_row. Internal sub-rows stack with height =
    /// self.height each. Must be the only entry in the row (panics otherwise).
    pub fn grid(mut self, widgets: Vec<Widget>, cols_per_row: usize) -> Self {
        assert!(cols_per_row > 0, "cols_per_row must be > 0");
        assert!(self.entries.is_empty(), "grid() must be the only entry in a row");
        self.entries.push(RowEntry::Grid { widgets, cols_per_row });
        self
    }
}

/// A collection of rows that stack vertically to form the full dashboard.
#[derive(Debug, Clone)]
pub struct Grid {
    rows: Vec<Row>,
}

impl Grid {
    /// Create an empty grid.
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Append a row to the grid.
    pub fn add_row(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// Flatten all rows into positioned tiles.
    ///
    /// Rows stack vertically: each row's y-offset equals the sum of all
    /// previous rows' effective heights. For `Grid` entries, internal sub-rows
    /// increment y by `row.height` per sub-row.
    pub fn to_tiles(&self) -> Vec<Tile> {
        let mut tiles = Vec::new();
        let mut y_offset: usize = 0;

        for row in &self.rows {
            let mut x_offset: usize = 0;
            let mut row_max_height = row.height;

            for entry in &row.entries {
                match entry {
                    RowEntry::Span { cols, widget } => {
                        tiles.push(Tile {
                            x: x_offset,
                            y: y_offset,
                            width: *cols,
                            height: row.height,
                            widget: widget.clone(),
                        });
                        x_offset += cols;
                    }
                    RowEntry::Grid { widgets, cols_per_row } => {
                        let cell_w = 12 / cols_per_row;
                        for (i, widget) in widgets.iter().enumerate() {
                            let gx = (i % cols_per_row) * cell_w;
                            let gy = y_offset + (i / cols_per_row) * row.height;
                            tiles.push(Tile {
                                x: gx,
                                y: gy,
                                width: cell_w,
                                height: row.height,
                                widget: widget.clone(),
                            });
                        }
                        let num_sub_rows = widgets.len().div_ceil(*cols_per_row);
                        let grid_height = num_sub_rows * row.height;
                        if grid_height > row_max_height {
                            row_max_height = grid_height;
                        }
                    }
                }
            }

            y_offset += row_max_height;
        }

        tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(name: &str) -> Widget {
        Widget::text(name, "")
    }

    #[test]
    fn single_full_width_tile() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(4).full(w("A")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0], Tile { x: 0, y: 0, width: 12, height: 4, widget: w("A") });
    }

    #[test]
    fn two_half_width_tiles_same_row() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(3).half(w("L")).half(w("R")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0], Tile { x: 0, y: 0, width: 6, height: 3, widget: w("L") });
        assert_eq!(tiles[1], Tile { x: 6, y: 0, width: 6, height: 3, widget: w("R") });
    }

    #[test]
    fn rows_stack_vertically() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(5).full(w("top")));
        grid.add_row(Row::new(3).full(w("bottom")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].y, 0);
        assert_eq!(tiles[1].y, 5);
    }

    #[test]
    fn grid_of_alert_charts() {
        let widgets: Vec<Widget> = (0..5).map(|i| w(&format!("alert-{}", i))).collect();
        let mut grid = Grid::new();
        grid.add_row(Row::new(3).grid(widgets, 4));
        let tiles = grid.to_tiles();
        // 5 widgets at 4-per-row = 2 sub-rows (4 + 1)
        assert_eq!(tiles.len(), 5);
        // Cell width = 12/4 = 3
        assert_eq!(tiles[0], Tile { x: 0, y: 0, width: 3, height: 3, widget: w("alert-0") });
        assert_eq!(tiles[1], Tile { x: 3, y: 0, width: 3, height: 3, widget: w("alert-1") });
        assert_eq!(tiles[2], Tile { x: 6, y: 0, width: 3, height: 3, widget: w("alert-2") });
        assert_eq!(tiles[3], Tile { x: 9, y: 0, width: 3, height: 3, widget: w("alert-3") });
        assert_eq!(tiles[4], Tile { x: 0, y: 3, width: 3, height: 3, widget: w("alert-4") });
    }

    #[test]
    fn empty_grid_produces_no_tiles() {
        let grid = Grid::new();
        assert!(grid.to_tiles().is_empty());
    }

    #[test]
    fn third_width_tiles() {
        let mut grid = Grid::new();
        grid.add_row(
            Row::new(2)
                .span(4, w("a"))
                .span(4, w("b"))
                .span(4, w("c")),
        );
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0], Tile { x: 0, y: 0, width: 4, height: 2, widget: w("a") });
        assert_eq!(tiles[1], Tile { x: 4, y: 0, width: 4, height: 2, widget: w("b") });
        assert_eq!(tiles[2], Tile { x: 8, y: 0, width: 4, height: 2, widget: w("c") });
    }
}
