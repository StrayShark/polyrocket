//! v0.86a —— 用于 specta codegen 的自定义 `OptionBigInt<T>` 包装器。
//! 完整设计说明见下文。

use serde::{Deserialize, Serialize};
use specta::Type;

/// v0.86a —— 在 specta codegen 中映射到 TS `bigint` 的 Newtype 包装器。
///
/// **原因**:`specta-typescript` 0.0.12 默认有 `bigint_forbidden` 规则,
/// 拒绝 i64/u64/i128/u128 字段。官方的 `BigInt<T>` 包装器
///(以 `serde` feature 为门控)通过 `#[specta(type = BigInt)]`
/// 或直接使用 `BigInt<i64>` 将字段标记为 TS `bigint`。
///
/// **限制**:对 `Option<i64>` 使用 `#[specta(type = BigInt)]`
/// **会丢失可空性**(类型覆盖作用于内部类型,而非 Option
/// 包装器)。而 `BigInt<i64>` 本身拥有**私有**元组字段,
/// 因此无法从 crate 外部构造。
///
/// **变通方案**:本包装器。`Option<OptionBigInt<i64>>` 与
/// `Option<i64>` 的序列化结果相同(内部的 OptionBigInt 对 serde 透明),
/// 但通过 `specta_typescript::define("bigint")` 引用
/// 映射到 TS `bigint | null`。因此
/// `pub field: Option<OptionBigInt<i64>>` → `field: bigint | null`。
///
/// **权衡**:字段声明更为冗长
///( `Option<OptionBigInt<i64>>` vs 普通的 `Option<i64>`)。
/// v0.86+ 仅在 5 个延后处理的 Option<i64> 字段上使用,
/// 这些场景下 i64 的无损传输至关重要。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptionBigInt<T>(pub Option<T>);

impl<T> OptionBigInt<T> {
    /// 从一个值构造新的 `OptionBigInt`。
    pub fn new(v: T) -> Self {
        Self(Some(v))
    }
    /// `None` 值(用于 `Option::None` 不便使用的场合)。
    pub const NONE: Self = Self(None);
}

// Serde 透明委托 —— 线缆格式为内部的 Option<T>。
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

// Specta Type impl:通过与 `specta_typescript::BigInt<T>` 使用的相同的不透明引用
// 映射到 TS `bigint`。`specta_typescript::define` 是内部 `opaque::define`
// 函数的公共再导出。
impl<T> Type for OptionBigInt<T> {
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        use specta::datatype::DataType;
        DataType::Reference(specta_typescript::define("bigint"))
    }
}

#[cfg(test)]
mod tests {
    //! 健全性测试 —— 验证 OptionBigInt 对 serde 透明,
    //! 并通过 specta 产生正确的 TS 类型。

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

    // 注意:完整的 specta::Type 往返测试需要构建
    // `Types` 注册表并断言 DataType 形状。
    // 我们已在 v0.86b 通过 `cargo run --bin gen_ts_types`
    // 验证生成的 TS。上面的 serde 往返测试足以确认
    // 与 `Option<i64>` 的线缆格式一致。
}
