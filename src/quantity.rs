//! Units and quantities: the pure conversion layer under the grocery list.
//!
//! Recipes may express an ingredient in any supported unit ("2 tbsp", "1 can",
//! "100 g"); the catalog fixes one **shopping unit** per ingredient (a base unit:
//! `g`, `ml` or `pcs`) plus an optional piece conversion. Everything here is plain
//! data and arithmetic — no Leptos, no catalog lookups — so it is trivially
//! unit-testable and reusable by both the recipe page and the shopping list.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What kind of thing a unit measures. Quantities only add within a dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Dimension {
    Mass,
    Volume,
    Count,
}

/// Every unit a recipe or the catalog may use. Base units (one per dimension)
/// are `G`, `Ml` and `Piece`; the rest convert into them with a fixed factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    G,
    Kg,
    Ml,
    L,
    Tsp,
    Tbsp,
    Cup,
    Piece,
}

impl Unit {
    pub fn dimension(self) -> Dimension {
        match self {
            Unit::G | Unit::Kg => Dimension::Mass,
            Unit::Ml | Unit::L | Unit::Tsp | Unit::Tbsp | Unit::Cup => Dimension::Volume,
            Unit::Piece => Dimension::Count,
        }
    }

    /// Multiplier into the dimension's base unit (g, ml or pieces).
    pub fn to_base(self) -> f64 {
        match self {
            Unit::G | Unit::Ml | Unit::Piece => 1.0,
            Unit::Kg | Unit::L => 1000.0,
            Unit::Tsp => 5.0,
            Unit::Tbsp => 15.0,
            Unit::Cup => 240.0,
        }
    }

    /// True for the units the catalog may use as a shopping unit.
    /// (Enforced by the data validation tests.)
    #[cfg(test)]
    pub fn is_base(self) -> bool {
        matches!(self, Unit::G | Unit::Ml | Unit::Piece)
    }

    /// Canonical spelling, used for display and serialization.
    pub fn as_str(self) -> &'static str {
        match self {
            Unit::G => "g",
            Unit::Kg => "kg",
            Unit::Ml => "ml",
            Unit::L => "l",
            Unit::Tsp => "tsp",
            Unit::Tbsp => "tbsp",
            Unit::Cup => "cup",
            Unit::Piece => "pcs",
        }
    }

    /// Parse a unit as written in JSON. Case-insensitive, a few common aliases,
    /// otherwise strict (no `"gr"`, no legacy `"unit"`) so typos fail validation.
    pub fn parse(s: &str) -> Option<Unit> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "g" | "gram" | "grams" => Unit::G,
            "kg" | "kilogram" | "kilograms" => Unit::Kg,
            "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => Unit::Ml,
            "l" | "liter" | "liters" | "litre" | "litres" => Unit::L,
            "tsp" | "teaspoon" | "teaspoons" => Unit::Tsp,
            "tbsp" | "tablespoon" | "tablespoons" => Unit::Tbsp,
            "cup" | "cups" => Unit::Cup,
            "pc" | "pcs" | "piece" | "pieces" => Unit::Piece,
            _ => return None,
        })
    }
}

impl Serialize for Unit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Unit::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown unit `{s}`")))
    }
}

/// An amount in a unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub amount: f64,
    pub unit: Unit,
}

impl Quantity {
    pub fn new(amount: f64, unit: Unit) -> Self {
        Self { amount, unit }
    }

    pub fn dimension(&self) -> Dimension {
        self.unit.dimension()
    }

    /// The amount expressed in the dimension's base unit.
    pub fn in_base(&self) -> f64 {
        self.amount * self.unit.to_base()
    }
}

/// How an ingredient is counted in pieces. Declared per catalog entry.
///
/// `label` names one piece ("clove", "can", "slice"); recipes may use it (or its
/// plural) as the unit. `per_piece` is how much of the ingredient's shopping unit
/// one piece is (150 g for an onion, 400 g for a can). For an ingredient shopped
/// in `pcs`, `per_piece` is instead the mass in grams of one piece, so recipes
/// can still say "100 g".
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CountSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plural: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_piece: Option<f64>,
}

impl CountSpec {
    /// Plural form of the label (explicit, or label + "s").
    pub fn plural_label(&self) -> Option<String> {
        self.plural
            .clone()
            .or_else(|| self.label.as_ref().map(|l| format!("{l}s")))
    }

    /// True when `s` is this ingredient's piece label (singular or plural).
    pub fn matches_label(&self, s: &str) -> bool {
        let s = s.trim();
        self.label
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case(s))
            || self
                .plural_label()
                .is_some_and(|p| p.eq_ignore_ascii_case(s))
    }

    /// The label to print after `n` pieces, if the ingredient has one.
    pub fn label_for(&self, n: f64) -> Option<String> {
        if (n - 1.0).abs() < 1e-9 {
            self.label.clone()
        } else {
            self.plural_label()
        }
    }
}

/// Why a recipe's unit string could not be understood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownUnit(pub String);

/// Resolve the unit a recipe wrote (or omitted) for an ingredient whose shopping
/// unit is `shopping_unit` and whose piece spec is `count`.
pub fn resolve_unit(
    raw: Option<&str>,
    shopping_unit: Unit,
    count: Option<&CountSpec>,
) -> Result<Unit, UnknownUnit> {
    match raw {
        None => Ok(shopping_unit),
        Some(s) => match Unit::parse(s) {
            Some(u) => Ok(u),
            None if count.is_some_and(|c| c.matches_label(s)) => Ok(Unit::Piece),
            None => Err(UnknownUnit(s.to_string())),
        },
    }
}

/// Result of converting a quantity into an ingredient's shopping unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Converted {
    /// Expressed in the shopping unit's base (summable with other contributions).
    Primary(f64),
    /// No conversion available: kept in its own dimension's base unit.
    Other(Dimension, f64),
}

/// Convert `q` into the ingredient's shopping unit where possible.
///
/// - Same dimension as the shopping unit: straight base conversion.
/// - Pieces with a `per_piece`: `amount × per_piece` shopping units.
/// - Mass into a `pcs`-shopped ingredient with `per_piece` grams: `grams ÷ per_piece`.
/// - Anything else: [`Converted::Other`], so the caller can show it side by side.
pub fn to_shopping(q: Quantity, shopping_unit: Unit, count: Option<&CountSpec>) -> Converted {
    let target = shopping_unit.dimension();
    let from = q.dimension();
    if from == target {
        return Converted::Primary(q.in_base());
    }
    let per_piece = count.and_then(|c| c.per_piece).filter(|p| *p > 0.0);
    match (from, target, per_piece) {
        (Dimension::Count, _, Some(p)) => Converted::Primary(q.amount * p),
        (Dimension::Mass, Dimension::Count, Some(p)) => Converted::Primary(q.in_base() / p),
        _ => Converted::Other(from, q.in_base()),
    }
}

/// Is `unit` acceptable for an ingredient with this shopping unit and piece
/// spec, i.e. will [`to_shopping`] always yield [`Converted::Primary`]?
/// (Used by the data validation tests.)
#[cfg(test)]
pub fn is_convertible(unit: Unit, shopping_unit: Unit, count: Option<&CountSpec>) -> bool {
    matches!(
        to_shopping(Quantity::new(1.0, unit), shopping_unit, count),
        Converted::Primary(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clove() -> CountSpec {
        CountSpec {
            label: Some("clove".into()),
            plural: None,
            per_piece: Some(4.0),
        }
    }

    #[test]
    fn parse_accepts_aliases_and_rejects_typos() {
        for (s, u) in [
            ("g", Unit::G),
            ("Grams", Unit::G),
            ("kg", Unit::Kg),
            ("ml", Unit::Ml),
            ("litre", Unit::L),
            ("tsp", Unit::Tsp),
            ("Tablespoons", Unit::Tbsp),
            ("cups", Unit::Cup),
            (" pcs ", Unit::Piece),
            ("piece", Unit::Piece),
        ] {
            assert_eq!(Unit::parse(s), Some(u), "{s}");
            assert_eq!(Unit::parse(u.as_str()), Some(u), "round trip {s}");
        }
        for bad in ["gr", "unit", "clove", "", "oz"] {
            assert_eq!(Unit::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn serde_round_trips_through_canonical_spelling() {
        assert_eq!(serde_json::to_string(&Unit::Piece).unwrap(), "\"pcs\"");
        assert_eq!(
            serde_json::from_str::<Unit>("\"Tbsp\"").unwrap(),
            Unit::Tbsp
        );
        assert!(serde_json::from_str::<Unit>("\"unit\"").is_err());
    }

    #[test]
    fn base_conversion_factors() {
        assert_eq!(Quantity::new(2.0, Unit::Tbsp).in_base(), 30.0);
        assert_eq!(Quantity::new(1.5, Unit::Kg).in_base(), 1500.0);
        assert_eq!(Quantity::new(0.5, Unit::L).in_base(), 500.0);
        assert_eq!(Quantity::new(1.0, Unit::Cup).in_base(), 240.0);
        assert_eq!(Quantity::new(3.0, Unit::Piece).in_base(), 3.0);
        for u in [Unit::G, Unit::Ml, Unit::Piece] {
            assert!(u.is_base());
            assert_eq!(u.to_base(), 1.0);
        }
        assert!(!Unit::Kg.is_base());
    }

    #[test]
    fn resolve_unit_handles_default_standard_and_labels() {
        assert_eq!(resolve_unit(None, Unit::G, None), Ok(Unit::G));
        assert_eq!(resolve_unit(Some("kg"), Unit::G, None), Ok(Unit::Kg));
        assert_eq!(
            resolve_unit(Some("cloves"), Unit::G, Some(&clove())),
            Ok(Unit::Piece)
        );
        assert_eq!(
            resolve_unit(Some("Clove"), Unit::G, Some(&clove())),
            Ok(Unit::Piece)
        );
        assert_eq!(
            resolve_unit(Some("clove"), Unit::G, None),
            Err(UnknownUnit("clove".into()))
        );
    }

    #[test]
    fn to_shopping_converts_or_keeps_dimension() {
        // Same dimension → base.
        assert_eq!(
            to_shopping(Quantity::new(2.0, Unit::Tbsp), Unit::Ml, None),
            Converted::Primary(30.0)
        );
        // Pieces with per_piece → shopping unit.
        assert_eq!(
            to_shopping(Quantity::new(3.0, Unit::Piece), Unit::G, Some(&clove())),
            Converted::Primary(12.0)
        );
        // Mass into a pcs-shopped ingredient.
        let egg = CountSpec {
            label: None,
            plural: None,
            per_piece: Some(50.0),
        };
        assert_eq!(
            to_shopping(Quantity::new(100.0, Unit::G), Unit::Piece, Some(&egg)),
            Converted::Primary(2.0)
        );
        // No conversion → other dimension, base units.
        assert_eq!(
            to_shopping(Quantity::new(2.0, Unit::Piece), Unit::G, None),
            Converted::Other(Dimension::Count, 2.0)
        );
        assert_eq!(
            to_shopping(Quantity::new(1.0, Unit::Cup), Unit::G, None),
            Converted::Other(Dimension::Volume, 240.0)
        );
        assert!(is_convertible(Unit::Piece, Unit::G, Some(&clove())));
        assert!(!is_convertible(Unit::Piece, Unit::G, None));
    }

    #[test]
    fn count_labels_pluralise() {
        let c = clove();
        assert_eq!(c.label_for(1.0).as_deref(), Some("clove"));
        assert_eq!(c.label_for(3.0).as_deref(), Some("cloves"));
        let leaf = CountSpec {
            label: Some("leaf".into()),
            plural: Some("leaves".into()),
            per_piece: None,
        };
        assert!(leaf.matches_label("LEAVES"));
        assert_eq!(leaf.label_for(2.0).as_deref(), Some("leaves"));
        assert_eq!(CountSpec::default().label_for(2.0), None);
    }
}
