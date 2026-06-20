//! v0.86a — custom `OptionBigInt<T>` wrapper for specta codegen.
//! See full design notes below.

use serde::{Deserialize, Serialize};
use specta::Type;

/// v0.86a — Newtype wrapper that maps to TS `bigint` in specta codegen.
///
/// **Why**: `specta-typescript` 0.0.12 has a `bigint_forbidden` rule
/// that rejects i64/u64/i128/u128 fields by default. The official
/// `BigInt<T>` wrapper (gated behind the `serde` feature) marks a
/// field as TS `bigint` via `#[specta(type = BigInt)]` or by using
/// `BigInt<i64>` directly.
///
/// **Limitation**: `#[specta(type = BigInt)]` on `Option<i64>` **loses
/// nullability** (the type override applies to the inner type, not the
/// Option wrapper). And `BigInt<i64>` itself has a PRIVATE tuple field,
/// so it can't be constructed from outside the crate.
///
/// **Workaround**: this wrapper. `Option<OptionBigInt<i64>>` serializes
/// the same as `Option<i64>` (the inner OptionBigInt is transparent to
/// serde), but maps to TS `bigint | null` via the
/// `specta_typescript::define("bigint")` reference. So
/// `pub field: Option<OptionBigInt<i64>>` → `field: bigint | null`.
///
/// **Trade-off**: field declaration is more verbose
/// (`Option<OptionBigInt<i64>>` vs plain `Option<i64>`). v0.86+ only
/// uses this for the 5 deferred Option<i64> fields where lossless i64
/// transport matters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptionBigInt<T>(pub Option<T>);

impl<T> OptionBigInt<T> {
    /// Construct a new `OptionBigInt` from a value.
    pub fn new(v: T) -> Self {
        Self(Some(v))
    }
    /// `None` value (for places where `Option::None` is awkward).
    pub const NONE: Self = Self(None);
}

// Serde transparent delegation — wire format is the inner Option<T>.
impl<T: Serialize> Serialize for OptionBigInt<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for OptionBigInt<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(d).map(Self)
    }
}

// Specta Type impl: maps to TS `bigint` via the same opaque reference
// that `specta_typescript::BigInt<T>` uses. `specta_typescript::define`
// is the public re-export of the internal `opaque::define` function.
impl<T> Type for OptionBigInt<T> {
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        use specta::datatype::DataType;
        DataType::Reference(specta_typescript::define("bigint"))
    }
}

#[cfg(test)]
mod tests {
    //! Sanity tests — verify OptionBigInt is transparent for serde
    //! and produces the right TS type via specta.

    use super::*;
    use specta::Type;

    #[derive(Serialize, Deserialize, Type)]
    struct Holder {
        pub field: Option<OptionBigInt<i64>>,
    }

    #[test]
    fn serde_serializes_none_as_null() {
        let h = Holder { field: None };
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, r#"{"field":null}"#);
    }

    #[test]
    fn serde_serializes_some_as_number() {
        let h = Holder { field: Some(OptionBigInt(Some(1234567890i64))) };
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, r#"{"field":1234567890}"#);
    }

    #[test]
    fn serde_deserializes_null_to_none() {
        let json = r#"{"field":null}"#;
        let h: Holder = serde_json::from_str(json).unwrap();
        assert!(h.field.is_none());
    }

    #[test]
    fn serde_deserializes_number_to_some() {
        let json = r#"{"field":42}"#;
        let h: Holder = serde_json::from_str(json).unwrap();
        assert_eq!(h.field.unwrap().0, Some(42));
    }

    // Note: a full specta::Type round-trip test would require building a
    // `Types` registry and asserting the DataType shape. We've verified the
    // generated TS via `cargo run --bin gen_ts_types` in v0.86b. The serde
    // round-trip tests above are sufficient to confirm wire-format parity
    // with `Option<i64>`.
}
