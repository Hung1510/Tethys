//! Patch-specific game data.
//!
//! # Maintainer note
//! The numbers below are the values that change when Kuro rebalances echoes or
//! adds sets. They are kept in this one file on purpose: updating Tethys for a
//! new patch should never require touching the optimizer or the model. Verify
//! against a current data source (e.g. the community spreadsheet or the
//! in-game values) when a patch lands, and bump the `PATCH` constant.

use crate::model::Stat;

/// The game patch these tables were last verified against.
pub const PATCH: &str = "2.x (verify before release)";

/// The maximum value a single substat roll of `stat` can take.
///
/// Used to convert an absolute rolled value into a normalised "roll value"
/// (RV): `rv = value / max_roll`, so a substat at its ceiling counts as one
/// full roll. This is the standard currency community optimizers score in.
///
/// Flat HP/ATK/DEF scale with the roll but their ceilings are large; the
/// values here are representative and should be verified per patch.
pub fn max_substat_roll(stat: Stat) -> f32 {
    match stat {
        Stat::Hp => 580.0,
        Stat::Atk => 60.0,
        Stat::Def => 70.0,
        Stat::HpPct => 11.6,
        Stat::AtkPct => 11.6,
        Stat::DefPct => 14.7,
        Stat::CritRate => 10.5,
        Stat::CritDmg => 21.0,
        Stat::EnergyRegen => 12.4,
        Stat::BasicAtk => 11.6,
        Stat::HeavyAtk => 11.6,
        Stat::ResonanceSkill => 11.6,
        Stat::ResonanceLiberation => 11.6,
        // The stats below do not appear as substats in-game; return a sentinel
        // so a stray roll cannot dominate scoring.
        Stat::HealingBonus
        | Stat::Glacio
        | Stat::Fusion
        | Stat::Electro
        | Stat::Aero
        | Stat::Spectro
        | Stat::Havoc => f32::INFINITY,
    }
}

/// Whether `stat` is a legal substat (as opposed to a main-stat-only stat such
/// as an elemental damage bonus).
pub fn is_valid_substat(stat: Stat) -> bool {
    matches!(
        stat,
        Stat::Hp
            | Stat::Atk
            | Stat::Def
            | Stat::HpPct
            | Stat::AtkPct
            | Stat::DefPct
            | Stat::CritRate
            | Stat::CritDmg
            | Stat::EnergyRegen
            | Stat::BasicAtk
            | Stat::HeavyAtk
            | Stat::ResonanceSkill
            | Stat::ResonanceLiberation
    )
}

/// The pool of main stats available for a given echo cost. Used by the scanner
/// to sanity-check OCR results and by build specs to filter candidates.
pub fn main_stat_pool(cost: u8) -> &'static [Stat] {
    match cost {
        4 => &[
            Stat::CritRate,
            Stat::CritDmg,
            Stat::HpPct,
            Stat::AtkPct,
            Stat::DefPct,
            Stat::HealingBonus,
        ],
        3 => &[
            Stat::HpPct,
            Stat::AtkPct,
            Stat::DefPct,
            Stat::EnergyRegen,
            Stat::Glacio,
            Stat::Fusion,
            Stat::Electro,
            Stat::Aero,
            Stat::Spectro,
            Stat::Havoc,
        ],
        1 => &[
            Stat::HpPct,
            Stat::AtkPct,
            Stat::DefPct,
            Stat::Hp,
            Stat::Atk,
            Stat::Def,
        ],
        _ => &[],
    }
}
