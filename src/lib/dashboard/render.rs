//! YAML renderer for the Cloud Monitoring mosaic dashboard.
//!
//! Takes a display name and a `Grid` (from the layout module), resolves tile
//! positions via `grid.to_tiles()`, and serializes the full dashboard
//! definition to a YAML string suitable for `gcloud monitoring dashboards
//! create --config-from-file`.

use serde_yaml::{Mapping, Value};

use crate::lib::dashboard::layout::{Grid, Tile};

/// Render a complete Cloud Monitoring dashboard YAML from the display name
/// and a positioned grid of widgets.
///
/// The output conforms to the `google.monitoring.dashboard.v1.Dashboard`
/// resource format expected by `gcloud monitoring dashboards create`.
pub fn render_dashboard_yaml(display_name: &str, grid: &Grid) -> String {
    let tiles = grid.to_tiles();

    let mut root = Mapping::new();
    root.insert(Value::from("displayName"), Value::from(display_name));

    let mut mosaic = Mapping::new();
    mosaic.insert(Value::from("columns"), Value::from(12u64));

    let tile_values: Vec<Value> = tiles.iter().map(tile_to_value).collect();
    mosaic.insert(Value::from("tiles"), Value::Sequence(tile_values));

    root.insert(Value::from("mosaicLayout"), Value::Mapping(mosaic));

    serde_yaml::to_string(&Value::Mapping(root))
        .expect("dashboard YAML serialization from in-memory Values should never fail")
}

/// Convert a single positioned tile into a serde_yaml::Value mapping.
///
/// Fields with value 0 (`xPos`, `yPos`) are omitted to keep the output
/// compact — the Cloud Monitoring API treats absent position fields as 0.
fn tile_to_value(tile: &Tile) -> Value {
    let mut map = Mapping::new();

    if tile.x > 0 {
        map.insert(Value::from("xPos"), Value::from(tile.x as u64));
    }
    if tile.y > 0 {
        map.insert(Value::from("yPos"), Value::from(tile.y as u64));
    }
    map.insert(Value::from("width"), Value::from(tile.width as u64));
    map.insert(Value::from("height"), Value::from(tile.height as u64));
    map.insert(Value::from("widget"), tile.widget.to_yaml_value());

    Value::Mapping(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::dashboard::layout::{Grid, Row};
    use crate::lib::dashboard::widgets::Widget;

    #[test]
    fn render_produces_valid_yaml_with_display_name() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(4).full(Widget::text("Test Widget", "")));

        let yaml_str = render_dashboard_yaml("My Dashboard", &grid);
        let parsed: Value = serde_yaml::from_str(&yaml_str)
            .expect("render should produce valid YAML");

        // Check displayName
        let root = parsed.as_mapping().unwrap();
        assert_eq!(
            root.get(&Value::from("displayName")),
            Some(&Value::from("My Dashboard"))
        );

        // Check mosaicLayout.columns == 12
        let mosaic = root
            .get(&Value::from("mosaicLayout"))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            mosaic.get(&Value::from("columns")),
            Some(&Value::from(12u64))
        );

        // Check that tiles array is non-empty
        let tiles = mosaic
            .get(&Value::from("tiles"))
            .unwrap()
            .as_sequence()
            .unwrap();
        assert_eq!(tiles.len(), 1);
    }

    #[test]
    fn tiles_have_correct_position_fields() {
        // Two half-width widgets in a single row: first at x=0, second at x=6
        let mut grid = Grid::new();
        grid.add_row(
            Row::new(4)
                .half(Widget::text("Left", ""))
                .half(Widget::text("Right", "")),
        );

        let yaml_str = render_dashboard_yaml("Position Test", &grid);
        let parsed: Value = serde_yaml::from_str(&yaml_str)
            .expect("render should produce valid YAML");

        let mosaic = parsed
            .as_mapping()
            .unwrap()
            .get(&Value::from("mosaicLayout"))
            .unwrap()
            .as_mapping()
            .unwrap();
        let tiles = mosaic
            .get(&Value::from("tiles"))
            .unwrap()
            .as_sequence()
            .unwrap();

        assert_eq!(tiles.len(), 2);

        // First tile: x=0, y=0 — both should be omitted
        let tile0 = tiles[0].as_mapping().unwrap();
        assert!(
            tile0.get(&Value::from("xPos")).is_none(),
            "xPos=0 should be omitted"
        );
        assert!(
            tile0.get(&Value::from("yPos")).is_none(),
            "yPos=0 should be omitted"
        );
        assert_eq!(
            tile0.get(&Value::from("width")),
            Some(&Value::from(6u64))
        );

        // Second tile: x=6, y=0
        let tile1 = tiles[1].as_mapping().unwrap();
        assert_eq!(
            tile1.get(&Value::from("xPos")),
            Some(&Value::from(6u64))
        );
        assert!(
            tile1.get(&Value::from("yPos")).is_none(),
            "yPos=0 should be omitted"
        );
        assert_eq!(
            tile1.get(&Value::from("width")),
            Some(&Value::from(6u64))
        );
    }

    #[test]
    fn empty_grid_renders_minimal_dashboard() {
        let grid = Grid::new();

        let yaml_str = render_dashboard_yaml("Empty Dashboard", &grid);
        let parsed: Value = serde_yaml::from_str(&yaml_str)
            .expect("empty grid should still produce valid YAML");

        let root = parsed.as_mapping().unwrap();
        assert_eq!(
            root.get(&Value::from("displayName")),
            Some(&Value::from("Empty Dashboard"))
        );

        let mosaic = root
            .get(&Value::from("mosaicLayout"))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            mosaic.get(&Value::from("columns")),
            Some(&Value::from(12u64))
        );

        // tiles should be an empty sequence
        let tiles = mosaic
            .get(&Value::from("tiles"))
            .unwrap()
            .as_sequence()
            .unwrap();
        assert!(tiles.is_empty());
    }
}
