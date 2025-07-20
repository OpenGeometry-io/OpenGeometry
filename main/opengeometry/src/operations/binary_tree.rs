use crate::{primitives::polygon::OGPolygon, utility::geometry::Geometry};

pub struct Binary2DNode {
    pub polygon: Option<OGPolygon>,
    pub left: Option<Box<Binary2DNode>>,
    pub right: Option<Box<Binary2DNode>>,
}

impl Binary2DNode {
    pub fn new(polygon: Option<OGPolygon>) -> Self {
        Binary2DNode {
            polygon,
            left: None,
            right: None,
        }
    }

    pub fn build(&self) {

    }   
}

pub struct Binary2DTree {
    pub tree: Option<Binary2DNode>,
    pub polygons: Vec<OGPolygon>
}

impl Binary2DTree {
    pub fn new() -> Self {
        Binary2DTree {
            tree: None,
            polygons: Vec::new(),
        }
    }

    pub fn add_polygon(&mut self, polygon: OGPolygon) {
        self.polygons.push(polygon);
    }

    pub fn build_tree(&mut self) {
        if self.polygons.is_empty() {
            return;
        }
        
        let root_polygon = self.polygons[0].clone();
        let binary_tree: Binary2DNode = Binary2DNode::new(Some(root_polygon));

        binary_tree.build();
        self.tree = Some(binary_tree);
    }

    pub fn get_middle_index(&self, polygon: &OGPolygon) -> usize {
        let brep_data = polygon.get_brep_data();
        // convert string to Geometry
        let brep: Geometry = serde_json::from_str(&brep_data).unwrap();
        let faces = brep.get_faces();

        let indices = faces[0].clone();
        let len = indices.len();
        
        let i0_val = indices[0];
        let i1_val = indices[1];

        if i0_val < i1_val {
            return (i0_val + i1_val) as usize / 2;
        } else {
            return (i1_val + i0_val + len as u8) as usize / 2 % len;
        }
    }
}
