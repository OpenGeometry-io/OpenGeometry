pub mod geometry {
    pub mod boolean2d;
    pub mod geometrybuffer;
    pub mod offset2d;
    pub mod offset_regions;
    pub mod poly2d;
    pub mod triangle;
}

pub mod operations {
    pub mod extrude;
    pub mod offset;
    pub mod sweep;
    pub mod triangulate;
    pub mod windingsort;
}

pub mod utility {
    pub mod bgeometry;
}

pub mod primitives {
    pub mod arc;
    pub mod cuboid;
    pub mod curve;
    pub mod cylinder;
    pub mod line;
    pub mod polygon;
    pub mod polyline;
    pub mod rectangle;
    pub mod sphere;
    pub mod sweep;
    pub mod wedge;
}

pub mod spatial {
    pub mod placement;
}

pub mod booleans;
pub mod brep;
pub mod editor;
pub mod export;
pub mod freeform;
pub mod scenegraph;
