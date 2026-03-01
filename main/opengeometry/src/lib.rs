pub mod geometry {
    pub mod basegeometry;
    pub mod triangle;
}

pub mod operations {
    pub mod extrude;
    pub mod triangulate;
    pub mod windingsort;
}

pub mod utility {
    pub mod bgeometry;
    pub mod geometry;
}

pub mod primitives {
    pub mod arc;
    pub mod cuboid;
    pub mod cylinder;
    pub mod line;
    pub mod polygon;
    pub mod polyline;
    pub mod rectangle;
    pub mod wedge;
}

pub mod brep;
pub mod export;
pub mod scenegraph;

// v0.3.0
// mod brep_ds {
//   mod vertex;
//   // mod halfedge;
//   mod edge;
//   mod face;
//   pub mod brep;
// }
