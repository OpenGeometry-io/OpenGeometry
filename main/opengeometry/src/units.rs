//! Length unit & scale model (debt item D8).
//!
//! The kernel historically carried no unit: geometry lived in abstract scene
//! units and STEP/IFC export hardcoded `SI_UNIT(.METRE.)`, so an exported file
//! could not state what its numbers meant and tools imported parts at the
//! wrong size. A model now carries an explicit [`LengthUnit`]; exporters emit
//! the matching unit context (D9) so exchange is unit-correct.
//!
//! The conventional default is millimetres, matching mechanical CAD and the
//! STEP ecosystem.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Micrometre,
    Millimetre,
    Centimetre,
    Metre,
    Kilometre,
    Inch,
    Foot,
}

impl Default for LengthUnit {
    /// mm — the mechanical-CAD / STEP convention.
    fn default() -> Self {
        LengthUnit::Millimetre
    }
}

impl LengthUnit {
    /// Length of one unit expressed in metres. Exact for SI and for the
    /// internationally-defined inch (0.0254 m) and foot.
    pub fn metres_per_unit(self) -> f64 {
        match self {
            LengthUnit::Micrometre => 1.0e-6,
            LengthUnit::Millimetre => 1.0e-3,
            LengthUnit::Centimetre => 1.0e-2,
            LengthUnit::Metre => 1.0,
            LengthUnit::Kilometre => 1.0e3,
            LengthUnit::Inch => 0.0254,
            LengthUnit::Foot => 0.3048,
        }
    }

    /// The factor to multiply a length in `self` by to express it in `target`.
    pub fn conversion_factor_to(self, target: LengthUnit) -> f64 {
        self.metres_per_unit() / target.metres_per_unit()
    }

    /// Whether this unit is a metric/SI multiple of the metre (and so can be
    /// emitted as a STEP `SI_UNIT` with a prefix).
    pub fn is_si(self) -> bool {
        !matches!(self, LengthUnit::Inch | LengthUnit::Foot)
    }

    /// STEP `SI_UNIT` prefix token (including the surrounding dots), or `$` for
    /// the base metre. `None` for non-SI units (use a conversion-based unit).
    pub fn step_si_prefix(self) -> Option<&'static str> {
        match self {
            LengthUnit::Micrometre => Some(".MICRO."),
            LengthUnit::Millimetre => Some(".MILLI."),
            LengthUnit::Centimetre => Some(".CENTI."),
            LengthUnit::Metre => Some("$"),
            LengthUnit::Kilometre => Some(".KILO."),
            LengthUnit::Inch | LengthUnit::Foot => None,
        }
    }

    /// Lowercase name used in conversion-based unit definitions / IFC.
    pub fn name(self) -> &'static str {
        match self {
            LengthUnit::Micrometre => "micrometre",
            LengthUnit::Millimetre => "millimetre",
            LengthUnit::Centimetre => "centimetre",
            LengthUnit::Metre => "metre",
            LengthUnit::Kilometre => "kilometre",
            LengthUnit::Inch => "inch",
            LengthUnit::Foot => "foot",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_millimetre() {
        assert_eq!(LengthUnit::default(), LengthUnit::Millimetre);
    }

    #[test]
    fn conversions_are_consistent() {
        assert!(
            (LengthUnit::Metre.conversion_factor_to(LengthUnit::Millimetre) - 1000.0).abs()
                < 1.0e-9
        );
        assert!(
            (LengthUnit::Centimetre.conversion_factor_to(LengthUnit::Millimetre) - 10.0).abs()
                < 1.0e-9
        );
        assert!(
            (LengthUnit::Inch.conversion_factor_to(LengthUnit::Millimetre) - 25.4).abs() < 1.0e-9
        );
        // round trip
        let f = LengthUnit::Foot.conversion_factor_to(LengthUnit::Inch);
        assert!((f - 12.0).abs() < 1.0e-9);
    }

    #[test]
    fn si_classification() {
        assert!(LengthUnit::Millimetre.is_si());
        assert!(!LengthUnit::Inch.is_si());
        assert_eq!(LengthUnit::Millimetre.step_si_prefix(), Some(".MILLI."));
        assert_eq!(LengthUnit::Metre.step_si_prefix(), Some("$"));
        assert_eq!(LengthUnit::Inch.step_si_prefix(), None);
    }
}
