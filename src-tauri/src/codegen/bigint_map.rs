//! v0.86c —— 用于 specta codegen 的自定义 `BigIntMap<K, V>` 包装器。
//! 映射到 TS `{ [key: K]: bigint }`(仅支持 V 为 BigInt 风格的类型)。
//!
//! 与 `OptionBigInt` 模式相同:serde 透明,
//! specta Type impl 返回自定义的 DataType 形状。
//!
//! **原因**:对 `HashMap<K, V>` 字段使用 `#[specta(type = BigInt)]`
//! 不会递归到 value 类型(specta-typescript 0.0.12 的限制)。
//! 对于 `AuditRetentionViewCodegen.overrides: HashMap<String, i64>`,
//! 我们希望导出为 TS `{ [key: string]: bigint }`。

use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use specta::Type;

/// v0.86c —— 映射到 TS `{ [key: K]: bigint }` 的 Newtype 包装器。
///
/// Serde 透明(委托给内部的 HashMap<K, V>)。
/// Specta Type impl 返回 `DataType::Map(key, Reference(bigint))`,
/// 其中 key 类型通过 `K::definition()` 派生
///(因此 `HashMap<String, i64>` → key 为 `String`,value 为 `bigint`)。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BigIntMap<K, V>(pub HashMap<K, V>)
where
    K: Eq + Hash;

impl<K, V> BigIntMap<K, V>
where
    K: Eq + Hash,
{
    /// 从 HashMap 构造。
    pub fn new(m: HashMap<K, V>) -> Self {
        Self(m)
    }
    /// 获取内部的 HashMap。
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

// Serde 透明委托。
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

// Specta Type impl。key 类型来自 K 的 Type impl;
// value 类型硬编码为 `bigint`(本包装器仅用于 bigint 风格的 V)。
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
    //! 健全性测试 —— 验证 BigIntMap 对 serde 是透明的。

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
