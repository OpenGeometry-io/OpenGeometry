use crate::brep::CurveGeometry;
use crate::math::{Interval, Point3};

#[derive(Clone, Debug)]
pub(crate) struct SsiCurve {
    pub(crate) curve: u32,
    pub(super) pcurves: [u32; 2],
    pub(super) domain: Option<Interval>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SsiResult {
    pub(crate) curves: Vec<SsiCurve>,
    pub(crate) contacts: Vec<Point3>,
    pub(super) coincident: bool,
}

pub(super) enum GeometryResult {
    Empty,
    Curves(Vec<(CurveGeometry, Option<Interval>)>),
    CurvesAndContacts {
        curves: Vec<(CurveGeometry, Option<Interval>)>,
        contacts: Vec<Point3>,
    },
    Contact(Point3),
    Coincident,
}
