//! L3 — Consensus.
//!
//! Aggregates individual LLM [`crate::domain::llm::CallOutcome`]s into a
//! single side + strength verdict (used by the Daily Brief and signal UI).
//!
//! **Status (v0.3c): stub.** Public API surface is declared so L2 commands
//! can wire up imports. Real implementation lands when the M9 "Multi-LLM
//! consensus" milestone moves to active.

use serde::{Deserialize, Serialize};

/// Side the LLMs agree on (`"YES"` / `"NO"` / `"MAYBE"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusSide {
    Yes,
    No,
    Maybe,
}

impl ConsensusSide {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsensusSide::Yes => "YES",
            ConsensusSide::No => "NO",
            ConsensusSide::Maybe => "MAYBE",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "YES" => Some(ConsensusSide::Yes),
            "NO" => Some(ConsensusSide::No),
            "MAYBE" => Some(ConsensusSide::Maybe),
            _ => None,
        }
    }
}

/// Result of a consensus pass over N LLM answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consensus {
    pub side: ConsensusSide,
    /// Fraction of LLMs that agreed with the winning side (0..1).
    pub strength: f64,
    /// How many LLMs contributed to this consensus.
    pub n_models: usize,
    /// Average confidence across contributing LLMs.
    pub avg_confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_side_round_trip() {
        for s in [ConsensusSide::Yes, ConsensusSide::No, ConsensusSide::Maybe] {
            assert_eq!(ConsensusSide::parse(s.as_str()), Some(s));
        }
    }
}
