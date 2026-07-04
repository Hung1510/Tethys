//! Domain model for Wuthering Waves echoes and builds.
//!
//! The layout of a Wuthering Waves echo build is fixed: five echoes with the
//! cost layout `[4, 3, 3, 1, 1]` (twelve cost total). Slot 0 is the 4-cost,
//! slots 1 and 2 are the 3-costs, and slots 3 and 4 are the 1-costs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The fixed cost layout of an echo build.
pub const COST_LAYOUT: [u8; 5] = [4, 3, 3, 1, 1];

/// Every stat that can appear as a main stat or substat on an echo.
///
/// The exact numeric values a stat can roll live in [`crate::data`]. This enum
/// is intentionally exhaustive over the stats the optimizer needs to reason
/// about; add new variants here and update the tables in `data.rs` when Kuro
/// introduces new stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    // Flat stats
    Hp,
    Atk,
    Def,
    // Percentage stats
    HpPct,
    AtkPct,
    DefPct,
    // Combat stats
    CritRate,
    CritDmg,
    EnergyRegen,
    // Amplifiers
    BasicAtk,
    HeavyAtk,
    ResonanceSkill,
    ResonanceLiberation,
    HealingBonus,
    // Elemental damage bonuses
    Glacio,
    Fusion,
    Electro,
    Aero,
    Spectro,
    Havoc,
}

impl Stat {
    /// Whether the stat is expressed as a percentage (affects OCR parsing and
    /// display, and which roll table in `data.rs` applies).
    pub fn is_percent(self) -> bool {
        !matches!(self, Stat::Hp | Stat::Atk | Stat::Def)
    }
}

/// A single rolled stat: which stat, and its numeric value.
///
/// For percentage stats the value is stored as the number itself, e.g. a
/// 10.5% crit rate substat is `value: 10.5`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatRoll {
    pub stat: Stat,
    pub value: f32,
}

impl StatRoll {
    pub fn new(stat: Stat, value: f32) -> Self {
        Self { stat, value }
    }
}

/// Echo set (the two-piece / five-piece sonata effects).
///
/// This list covers the sets that existed through the 2.x patches. It is kept
/// deliberately flat and `#[non_exhaustive]` so new sets can be appended
/// without breaking downstream match arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EchoSet {
    FreezingFrost,
    MoltenRiftEmbers,
    VoidThunder,
    SierraGale,
    CelestialLight,
    SunSinkingEclipse,
    RejuvenatingGlow,
    MoonlitClouds,
    LingeringTunes,
    FrostyResolve,
    EternalRadiance,
    MidnightVeil,
    EmpyreanAnthem,
    TidebreakingCourage,
    /// Any set the optimizer does not specifically model.
    Other,
}

/// A single owned echo, as read from the game (via OCR) or imported from JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Echo {
    /// Stable id, unique within an [`Inventory`]. Used so the optimizer never
    /// places the same physical echo in two slots.
    pub id: u32,
    /// Informational: the monster the echo comes from (e.g. "Mourning Aix").
    #[serde(default)]
    pub name: String,
    pub set: EchoSet,
    /// 1, 3, or 4.
    pub cost: u8,
    /// Enhancement level, 0..=25.
    #[serde(default)]
    pub level: u8,
    pub main_stat: StatRoll,
    /// Up to five substats.
    pub substats: Vec<StatRoll>,
}

impl Echo {
    /// The slot index in [`COST_LAYOUT`] that this echo can occupy.
    /// Returns `None` for an invalid cost.
    pub fn slot_group(&self) -> Option<SlotGroup> {
        match self.cost {
            4 => Some(SlotGroup::Cost4),
            3 => Some(SlotGroup::Cost3),
            1 => Some(SlotGroup::Cost1),
            _ => None,
        }
    }
}

/// Groups of interchangeable slots by cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotGroup {
    Cost4,
    Cost3,
    Cost1,
}

/// A collection of owned echoes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub echoes: Vec<Echo>,
}

impl Inventory {
    pub fn new(echoes: Vec<Echo>) -> Self {
        Self { echoes }
    }

    pub fn len(&self) -> usize {
        self.echoes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.echoes.is_empty()
    }

    pub fn get(&self, id: u32) -> Option<&Echo> {
        self.echoes.iter().find(|e| e.id == id)
    }

    /// All echoes matching a slot group, in inventory order.
    pub fn by_group(&self, group: SlotGroup) -> impl Iterator<Item = &Echo> {
        self.echoes
            .iter()
            .filter(move |e| e.slot_group() == Some(group))
    }
}

/// A resolved five-echo build. Slots are ordered per [`COST_LAYOUT`].
#[derive(Debug, Clone, PartialEq)]
pub struct Build<'a> {
    pub slots: [&'a Echo; 5],
}

impl<'a> Build<'a> {
    /// Sum every stat across main stats and substats.
    pub fn total_stats(&self) -> HashMap<Stat, f32> {
        let mut totals: HashMap<Stat, f32> = HashMap::new();
        for echo in self.slots {
            *totals.entry(echo.main_stat.stat).or_insert(0.0) += echo.main_stat.value;
            for sub in &echo.substats {
                *totals.entry(sub.stat).or_insert(0.0) += sub.value;
            }
        }
        totals
    }

    /// Count how many echoes belong to each set (for two/five-piece bonuses).
    pub fn set_counts(&self) -> HashMap<EchoSet, u8> {
        let mut counts: HashMap<EchoSet, u8> = HashMap::new();
        for echo in self.slots {
            *counts.entry(echo.set).or_insert(0) += 1;
        }
        counts
    }

    /// The dominant set and how many pieces of it are present.
    pub fn dominant_set(&self) -> (EchoSet, u8) {
        self.set_counts()
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .unwrap_or((EchoSet::Other, 0))
    }
}
