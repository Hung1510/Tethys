//! Build evaluation.
//!
//! The optimizer is generic over an [`Evaluator`]. Version 0.1 ships the
//! standard community approach: score each substat by a per-character weight,
//! measured in normalised roll value. A damage-formula evaluator (base stats +
//! multipliers through the real damage equation) can be added later by
//! implementing the same trait, with no change to the optimizer.

use crate::data::max_substat_roll;
use crate::model::{Build, EchoSet, Stat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Anything that can assign a scalar score to a build. Higher is better.
pub trait Evaluator {
    fn score(&self, build: &Build) -> f32;
}

/// A named set of per-stat weights describing what a character wants.
///
/// Weights are multiplied by each substat's roll value (value / max_roll), so a
/// crit-rate substat rolled at its ceiling contributes exactly its weight. Main
/// stats are scored the same way via `main_stat_weight`, which lets the
/// optimizer prefer, say, a crit-rate 4-cost over an attack% one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterProfile {
    pub name: String,
    /// Weight per stat when it appears as a substat. Missing = 0.
    pub substat_weights: HashMap<Stat, f32>,
    /// Weight per stat when it appears as a main stat. Missing = 0.
    #[serde(default)]
    pub main_stat_weights: HashMap<Stat, f32>,
    /// Bonus added when the build reaches five pieces of this set.
    #[serde(default)]
    pub preferred_set: Option<EchoSet>,
    /// Flat score bonus for a full five-piece of `preferred_set`. Expressed in
    /// the same units as roll-value weight (a couple of rolls' worth is typical).
    #[serde(default)]
    pub set_bonus: f32,
}

impl CharacterProfile {
    /// A generic crit DPS profile: wants crit, attack%, and its damage
    /// amplifiers. A reasonable default when no tuned profile exists.
    pub fn generic_dps() -> Self {
        let substat_weights = HashMap::from([
            (Stat::CritRate, 1.0),
            (Stat::CritDmg, 1.0),
            (Stat::AtkPct, 0.75),
            (Stat::Atk, 0.4),
            (Stat::BasicAtk, 0.5),
            (Stat::HeavyAtk, 0.5),
            (Stat::ResonanceSkill, 0.5),
            (Stat::ResonanceLiberation, 0.5),
            (Stat::EnergyRegen, 0.25),
        ]);
        let main_stat_weights = HashMap::from([
            (Stat::CritRate, 1.0),
            (Stat::CritDmg, 1.0),
            (Stat::AtkPct, 0.75),
            (Stat::Glacio, 0.9),
            (Stat::Fusion, 0.9),
            (Stat::Electro, 0.9),
            (Stat::Aero, 0.9),
            (Stat::Spectro, 0.9),
            (Stat::Havoc, 0.9),
        ]);
        Self {
            name: "Generic DPS".into(),
            substat_weights,
            main_stat_weights,
            preferred_set: None,
            set_bonus: 0.0,
        }
    }

    /// An energy-regen support profile in the spirit of a Shorekeeper-style
    /// enabler: crit for the odd offensive echo, but ER and healing valued
    /// highly. Illustrative — tune against a real build guide before trusting.
    pub fn support_er() -> Self {
        let substat_weights = HashMap::from([
            (Stat::EnergyRegen, 1.0),
            (Stat::CritRate, 0.6),
            (Stat::CritDmg, 0.6),
            (Stat::HpPct, 0.4),
            (Stat::Hp, 0.2),
        ]);
        let main_stat_weights = HashMap::from([
            (Stat::EnergyRegen, 1.0),
            (Stat::CritRate, 0.7),
            (Stat::CritDmg, 0.7),
            (Stat::HealingBonus, 0.8),
            (Stat::HpPct, 0.5),
        ]);
        Self {
            name: "Support (Energy Regen)".into(),
            substat_weights,
            main_stat_weights,
            preferred_set: Some(EchoSet::RejuvenatingGlow),
            set_bonus: 2.0,
        }
    }
}

/// Scores builds by weighted normalised roll value using a [`CharacterProfile`].
pub struct WeightedSubstatEvaluator {
    profile: CharacterProfile,
}

impl WeightedSubstatEvaluator {
    pub fn new(profile: CharacterProfile) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> &CharacterProfile {
        &self.profile
    }
}

impl Evaluator for WeightedSubstatEvaluator {
    fn score(&self, build: &Build) -> f32 {
        let mut score = 0.0;

        for echo in build.slots {
            // Main stat.
            if let Some(w) = self.profile.main_stat_weights.get(&echo.main_stat.stat) {
                let rv = roll_value(echo.main_stat.stat, echo.main_stat.value);
                score += w * rv;
            }
            // Substats.
            for sub in &echo.substats {
                if let Some(w) = self.profile.substat_weights.get(&sub.stat) {
                    score += w * roll_value(sub.stat, sub.value);
                }
            }
        }

        // Five-piece set bonus.
        if let Some(preferred) = self.profile.preferred_set {
            if build.set_counts().get(&preferred).copied().unwrap_or(0) >= 5 {
                score += self.profile.set_bonus;
            }
        }

        score
    }
}

/// Convert an absolute stat value into a normalised roll value in ~[0, 1+].
/// Main stats can exceed 1.0 (they are larger than a single substat roll),
/// which is intentional: a maxed main stat should outweigh one substat roll.
fn roll_value(stat: Stat, value: f32) -> f32 {
    let max = max_substat_roll(stat);
    if max.is_finite() && max > 0.0 {
        value / max
    } else {
        // Stat has no substat ceiling (e.g. an elemental main stat). Fall back
        // to a fixed contribution so it still registers.
        1.0
    }
}
