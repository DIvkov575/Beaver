//! Widget definitions for the Cloud Monitoring dashboard mosaic layout.
//!
//! Each variant maps to a single Cloud Monitoring dashboard widget type.
//! The layout engine positions these on the grid; the render module
//! serializes them to YAML.

/// A single dashboard widget that occupies one tile in the mosaic grid.
#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    /// Placeholder — widget type TBD during parallel development.
    Placeholder { title: String },
}
