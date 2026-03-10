pub mod geometry {
    pub mod basegeometry;
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

pub mod brep;
pub mod export;
pub mod file_exporter;
pub mod scenegraph;
