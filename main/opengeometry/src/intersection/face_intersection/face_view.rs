use crate::brep::BrepEnvelope;

#[derive(Clone, Copy)]
pub(crate) struct FaceView<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) face: u32,
}
