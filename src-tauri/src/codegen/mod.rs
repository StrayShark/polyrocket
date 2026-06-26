//! codegen 支持类型 —— 帮助 `specta-typescript`
//! 为那些否则会被拒绝的字段(例如 i64/u64)
//! 生成正确 TS 类型的包装器。
//!
//! 每个子模块的设计原理请参见对应模块文档。

pub mod bigint_map;
pub mod option_bigint;
