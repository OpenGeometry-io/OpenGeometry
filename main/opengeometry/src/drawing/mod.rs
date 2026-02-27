/**
 * Drawing Abstraction Module
 * 
 * This module provides a neutral 2D drawing representation that is independent
 * of any specific export format (PDF, SVG, DXF) or rendering system.
 * 
 * All geometry exporters should target this representation.
 */

pub mod primitives;
pub mod scene;

pub use primitives::{Vec2, Segment2D, Path2D};
pub use scene::Scene2D;
