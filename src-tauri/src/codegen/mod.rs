//! Codegen support types — wrappers that help `specta-typescript`
//! generate correct TS types for fields that would otherwise be
//! rejected (e.g. i64/u64).
//!
//! See each submodule for design rationale.

pub mod bigint_map;
pub mod option_bigint;
