/**
 * 2D Drawing Primitives
 * 
 * Core types for representing 2D vector drawings.
 * These are pure geometric representations with no rendering logic.
 */

use serde::{Serialize, Deserialize};

/// A 2D point/vector
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn distance_to(&self, other: &Vec2) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn add(&self, other: &Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.y + other.y)
    }

    pub fn subtract(&self, other: &Vec2) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }

    pub fn scale(&self, factor: f64) -> Vec2 {
        Vec2::new(self.x * factor, self.y * factor)
    }
}

/// A 2D path segment
/// 
/// This enum is designed to be extensible for future segment types
/// like arcs, bezier curves, etc.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Segment2D {
    /// A straight line segment from start to end
    Line { start: Vec2, end: Vec2 },
    
    // Future extensions:
    // Arc { center: Vec2, radius: f64, start_angle: f64, end_angle: f64 },
    // QuadraticBezier { start: Vec2, control: Vec2, end: Vec2 },
    // CubicBezier { start: Vec2, control1: Vec2, control2: Vec2, end: Vec2 },
}

impl Segment2D {
    /// Create a new line segment
    pub fn line(start: Vec2, end: Vec2) -> Self {
        Segment2D::Line { start, end }
    }

    /// Get the start point of the segment
    pub fn start_point(&self) -> Vec2 {
        match self {
            Segment2D::Line { start, .. } => *start,
        }
    }

    /// Get the end point of the segment
    pub fn end_point(&self) -> Vec2 {
        match self {
            Segment2D::Line { end, .. } => *end,
        }
    }

    /// Get the length of the segment
    pub fn length(&self) -> f64 {
        match self {
            Segment2D::Line { start, end } => start.distance_to(end),
        }
    }
}

/// A 2D path consisting of connected segments
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Path2D {
    /// The segments that make up this path
    pub segments: Vec<Segment2D>,
    /// Whether the path forms a closed loop
    pub closed: bool,
    /// Optional stroke width in drawing units (meters)
    pub stroke_width: Option<f64>,
    /// Optional stroke color as RGB values (0.0 - 1.0)
    pub stroke_color: Option<(f64, f64, f64)>,
}

impl Path2D {
    /// Create a new empty path
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            closed: false,
            stroke_width: None,
            stroke_color: None,
        }
    }

    /// Create a new path with specified closure
    pub fn with_closed(closed: bool) -> Self {
        Self {
            segments: Vec::new(),
            closed,
            stroke_width: None,
            stroke_color: None,
        }
    }

    /// Add a line segment to the path
    pub fn add_line(&mut self, start: Vec2, end: Vec2) {
        self.segments.push(Segment2D::line(start, end));
    }

    /// Add a segment to the path
    pub fn add_segment(&mut self, segment: Segment2D) {
        self.segments.push(segment);
    }

    /// Set the stroke width
    pub fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = Some(width);
    }

    /// Set the stroke color (RGB, 0.0 - 1.0)
    pub fn set_stroke_color(&mut self, r: f64, g: f64, b: f64) {
        self.stroke_color = Some((r, g, b));
    }

    /// Check if the path is empty
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the number of segments
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Get the total length of all segments
    pub fn total_length(&self) -> f64 {
        self.segments.iter().map(|s| s.length()).sum()
    }

    /// Get the bounding box of the path as (min, max)
    pub fn bounding_box(&self) -> Option<(Vec2, Vec2)> {
        if self.segments.is_empty() {
            return None;
        }

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for segment in &self.segments {
            match segment {
                Segment2D::Line { start, end } => {
                    min_x = min_x.min(start.x).min(end.x);
                    min_y = min_y.min(start.y).min(end.y);
                    max_x = max_x.max(start.x).max(end.x);
                    max_y = max_y.max(start.y).max(end.y);
                }
            }
        }

        Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
    }

    /// Create a path from a sequence of points
    pub fn from_points(points: &[Vec2], closed: bool) -> Self {
        let mut path = Self::with_closed(closed);
        
        if points.len() < 2 {
            return path;
        }

        for i in 0..points.len() - 1 {
            path.add_line(points[i], points[i + 1]);
        }

        // Add closing segment if needed
        if closed && points.len() > 2 {
            path.add_line(points[points.len() - 1], points[0]);
        }

        path
    }
}

impl Default for Path2D {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_operations() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(4.0, 6.0);
        
        assert_eq!(a.add(&b), Vec2::new(5.0, 8.0));
        assert_eq!(b.subtract(&a), Vec2::new(3.0, 4.0));
        assert_eq!(a.scale(2.0), Vec2::new(2.0, 4.0));
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_path_from_points() {
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
        ];
        
        let open_path = Path2D::from_points(&points, false);
        assert_eq!(open_path.segment_count(), 2);
        assert!(!open_path.closed);

        let closed_path = Path2D::from_points(&points, true);
        assert_eq!(closed_path.segment_count(), 3);
        assert!(closed_path.closed);
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![
            Vec2::new(-1.0, -2.0),
            Vec2::new(3.0, 4.0),
            Vec2::new(1.0, 5.0),
        ];
        
        let path = Path2D::from_points(&points, false);
        let bbox = path.bounding_box().unwrap();
        
        assert_eq!(bbox.0, Vec2::new(-1.0, -2.0));
        assert_eq!(bbox.1, Vec2::new(3.0, 5.0));
    }
}
