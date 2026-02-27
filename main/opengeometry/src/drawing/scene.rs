/**
 * 2D Scene Container
 * 
 * A Scene2D holds multiple paths and provides operations for
 * managing and transforming the entire drawing.
 */

use super::primitives::{Path2D, Vec2, Segment2D};
use serde::{Serialize, Deserialize};

/// A 2D scene containing multiple paths
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scene2D {
    /// All paths in the scene
    paths: Vec<Path2D>,
    /// Optional scene name/title
    pub name: Option<String>,
    /// Drawing units (e.g., "meters", "millimeters")
    pub units: String,
}

impl Scene2D {
    /// Create a new empty scene with default units (meters)
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            name: None,
            units: "meters".to_string(),
        }
    }

    /// Create a new scene with a name
    pub fn with_name(name: &str) -> Self {
        Self {
            paths: Vec::new(),
            name: Some(name.to_string()),
            units: "meters".to_string(),
        }
    }

    /// Add a path to the scene
    pub fn add_path(&mut self, path: Path2D) {
        self.paths.push(path);
    }

    /// Get all paths in the scene
    pub fn paths(&self) -> &[Path2D] {
        &self.paths
    }

    /// Get mutable reference to all paths
    pub fn paths_mut(&mut self) -> &mut Vec<Path2D> {
        &mut self.paths
    }

    /// Get the number of paths
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Check if the scene is empty
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Get the bounding box of the entire scene
    pub fn bounding_box(&self) -> Option<(Vec2, Vec2)> {
        if self.paths.is_empty() {
            return None;
        }

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for path in &self.paths {
            if let Some((path_min, path_max)) = path.bounding_box() {
                min_x = min_x.min(path_min.x);
                min_y = min_y.min(path_min.y);
                max_x = max_x.max(path_max.x);
                max_y = max_y.max(path_max.y);
            }
        }

        if min_x == f64::MAX {
            return None;
        }

        Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
    }

    /// Get the width of the scene
    pub fn width(&self) -> f64 {
        self.bounding_box()
            .map(|(min, max)| max.x - min.x)
            .unwrap_or(0.0)
    }

    /// Get the height of the scene
    pub fn height(&self) -> f64 {
        self.bounding_box()
            .map(|(min, max)| max.y - min.y)
            .unwrap_or(0.0)
    }

    /// Clear all paths from the scene
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Translate all paths by an offset
    pub fn translate(&mut self, offset: Vec2) {
        for path in &mut self.paths {
            for segment in &mut path.segments {
                match segment {
                    Segment2D::Line { start, end } => {
                        *start = start.add(&offset);
                        *end = end.add(&offset);
                    }
                }
            }
        }
    }

    /// Scale all paths by a factor around the origin
    pub fn scale(&mut self, factor: f64) {
        for path in &mut self.paths {
            for segment in &mut path.segments {
                match segment {
                    Segment2D::Line { start, end } => {
                        *start = start.scale(factor);
                        *end = end.scale(factor);
                    }
                }
            }
        }
    }

    /// Normalize the scene to fit within a given size, centered at origin
    pub fn normalize_to_fit(&mut self, max_width: f64, max_height: f64) {
        if let Some((min, max)) = self.bounding_box() {
            let width = max.x - min.x;
            let height = max.y - min.y;

            if width <= 0.0 && height <= 0.0 {
                return;
            }

            // Calculate scale to fit
            let scale_x = if width > 0.0 { max_width / width } else { 1.0 };
            let scale_y = if height > 0.0 { max_height / height } else { 1.0 };
            let scale = scale_x.min(scale_y);

            // Center offset
            let center = Vec2::new(
                (min.x + max.x) / 2.0,
                (min.y + max.y) / 2.0,
            );

            // Translate to origin, scale, then translate to center of target
            self.translate(Vec2::new(-center.x, -center.y));
            self.scale(scale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_bounding_box() {
        let mut scene = Scene2D::new();
        
        let points1 = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)];
        let points2 = vec![Vec2::new(-5.0, 5.0), Vec2::new(5.0, 15.0)];
        
        scene.add_path(Path2D::from_points(&points1, false));
        scene.add_path(Path2D::from_points(&points2, false));
        
        let bbox = scene.bounding_box().unwrap();
        assert_eq!(bbox.0, Vec2::new(-5.0, 0.0));
        assert_eq!(bbox.1, Vec2::new(10.0, 15.0));
    }

    #[test]
    fn test_scene_translate() {
        let mut scene = Scene2D::new();
        let points = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)];
        scene.add_path(Path2D::from_points(&points, false));
        
        scene.translate(Vec2::new(5.0, 10.0));
        
        let bbox = scene.bounding_box().unwrap();
        assert_eq!(bbox.0, Vec2::new(5.0, 10.0));
        assert_eq!(bbox.1, Vec2::new(6.0, 11.0));
    }
}
