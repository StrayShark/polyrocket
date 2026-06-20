//! v0.86c — custom `BigIntMap<K, V>` wrapper for specta codegen.
//! Maps to TS `{ [key: K]: bigint }` (only V=BigInt-style types supported).
//!
//! Same pattern as `OptionBigInt`: serde transparent, specta Type
//! impl returns custom DataType shape.
//!
//! **Why**: `#[specta(type = BigInt)]` on a `HashMap<K, V>` field
//! doesn't recurse into the value type (specta-typescript 0.0.12
//! limitation). For `AuditRetentionViewCodegen.overrides:
//! HashMap<String, i64>` we want TS `{ [key: string]: bigint }`.

use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use specta::Type;

/// v0.86c — Newtype wrapper that maps to TS `{ [key: K]: bigint }`.
///
/// Serde is transparent (delegates to inner HashMap<K, V>). Specta
/// Type impl returns `DataType::Map(key, Reference(bigint))` where
/// the key type is derived from K via `K::definition()` (so
/// `HashMap<String, i64>` → key is `String`, value is `bigint`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BigIntMap<K, V>(pub HashMap<K, V>)
where
    K: Eq + Hash;

impl<K, V> BigIntMap<K, V>
where
    K: Eq + Hash,
{
    /// Construct from a HashMap.
    pub fn new(m: HashMap<K, V>) -> Self {
        Self(m)
    }
    /// Get the inner HashMap.
    pub fn inner(&self) -> &HashMap<K, V> {
        &self.0
    }
}

impl<K, V> From<HashMap<K, V>> for BigIntMap<K, V>
where
    K: Eq + Hash,
{
    fn from(m: HashMap<K, V>) -> Self {
        Self(m)
    }
}

// Serde transparent delegation.
impl<K, V> Serialize for BigIntMap<K, V>
where
    K: Eq + Hash + Serialize,
    V: Serialize,
{
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de, K, V> Deserialize<'de> for BigIntMap<K, V>
where
    K: Eq + Hash + Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        HashMap::<K, V>::deserialize(d).map(Self)
    }
}

// Specta Type impl. The key type comes from K's Type impl; the value
// type is hardcoded to `bigint` (this wrapper is for bigint-style V only).
impl<K, V> Type for BigIntMap<K, V>
where
    K: Eq + Hash + Type,
{
    fn definition(t: &mut specta::Types) -> specta::datatype::DataType {
        use specta::datatype::{DataType, Map};
        let key_ty = K::definition(t);
        let value_ty = DataType::Reference(specta_typescript::define("bigint"));
        DataType::Map(Map::new(key_ty, value_ty))
    }
}

#[cfg(test)]
mod tests {
    //! Sanity tests — verify BigIntMap is transparent for serde.

    use super::*;
    use std::collections::HashMap;

    #[test]
    fn serde_serializes_empty_as_empty_object() {
        let m: BigIntMap<String, i64> = BigIntMap::default();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn serde_serializes_entries_as_object() {
        let mut h: HashMap<String, i64> = HashMap::new();
        h.insert("a".to_string(), 1);
        h.insert("b".to_string(), 2);
        let m = BigIntMap::new(h);
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""a":1"#));
        assert!(json.contains(r#""b":2"#));
    }

    #[test]
    fn serde_round_trip() {
        let mut h: HashMap<String, i64> = HashMap::new();
        h.insert("x".to_string(), 100);
        let m = BigIntMap::new(h);
        let json = serde_json::to_string(&m).unwrap();
        let m2: BigIntMap<String, i64> = serde_json::from_str(&json).unwrap();
        assert_eq!(m2.inner().get("x"), Some(&100));
    }
}
