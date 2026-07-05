//! Tethys core — domain model, scoring, and optimization for Wuthering Waves
//! echo builds.
//!
//! This crate is platform-independent and has no I/O. The Windows-specific
//! screen capture and OCR live in the `scanner` crate; the GUI lives in `app`.
//! Everything here can be unit-tested on any platform, which is why the
//! algorithmically interesting parts (optimizer, OCR-text parsing) live here.

pub mod data;
pub mod model;
pub mod optimizer;
pub mod parse;
pub mod score;

/// Common imports.
pub mod prelude {
    pub use crate::model::{Build, Echo, EchoSet, Inventory, SlotGroup, Stat, StatRoll};
    pub use crate::optimizer::{
        optimize_exhaustive, optimize_ga, BuildSpec, GaConfig, OptimizeError, OptimizeResult,
    };
    pub use crate::parse::{
        infer_cost_from_main, parse_cost, parse_lines, parse_set_name, parse_substat_line,
    };
    pub use crate::score::{CharacterProfile, Evaluator, WeightedSubstatEvaluator};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;
    use crate::model::StatRoll;

    // --- parsing ------------------------------------------------------------

    #[test]
    fn parses_percent_and_flat_stats() {
        assert_eq!(
            parse_substat_line("Crit. Rate 6.3%"),
            Some(StatRoll::new(Stat::CritRate, 6.3))
        );
        assert_eq!(
            parse_substat_line("Crit. DMG 21.0%"),
            Some(StatRoll::new(Stat::CritDmg, 21.0))
        );
        // ATK is ambiguous: percent vs flat decided by the '%'.
        assert_eq!(
            parse_substat_line("ATK 9.4%"),
            Some(StatRoll::new(Stat::AtkPct, 9.4))
        );
        assert_eq!(
            parse_substat_line("ATK 40"),
            Some(StatRoll::new(Stat::Atk, 40.0))
        );
    }

    #[test]
    fn parses_messy_ocr_variants() {
        assert_eq!(
            parse_substat_line("Energy Regen  12.4%"),
            Some(StatRoll::new(Stat::EnergyRegen, 12.4))
        );
        assert_eq!(
            parse_substat_line("Resonance Liberation DMG Bonus 10.0%"),
            Some(StatRoll::new(Stat::ResonanceLiberation, 10.0))
        );
        // Junk lines return None rather than a bogus stat.
        assert_eq!(parse_substat_line("Cost 4"), None);
        assert_eq!(parse_substat_line("Mourning Aix"), None);
        assert_eq!(parse_substat_line(""), None);
    }

    #[test]
    fn parse_lines_filters_noise() {
        let block = [
            "Sinking Eclipse",
            "Crit. Rate 10.5%",
            "some header",
            "ATK 30",
            "DEF 40",
        ];
        let stats = parse_lines(block);
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[0].stat, Stat::CritRate);
    }

    #[test]
    fn parses_cost_only_from_valid_values() {
        assert_eq!(parse_cost("COST 4"), Some(4));
        assert_eq!(parse_cost("3"), Some(3));
        assert_eq!(parse_cost("Cost: 1"), Some(1));
        // A 44.0% main stat leaking into the region must not read as a cost.
        assert_eq!(parse_cost("Crit. DMG 44.0%"), None);
        assert_eq!(parse_cost("no digits here"), None);
    }

    #[test]
    fn infers_cost_from_unambiguous_mains() {
        assert_eq!(infer_cost_from_main(Stat::CritDmg), Some(4));
        assert_eq!(infer_cost_from_main(Stat::Fusion), Some(3));
        assert_eq!(infer_cost_from_main(Stat::EnergyRegen), Some(3));
        assert_eq!(infer_cost_from_main(Stat::Atk), Some(1));
        // Percentage mains appear at several costs — ambiguous.
        assert_eq!(infer_cost_from_main(Stat::AtkPct), None);
    }

    #[test]
    fn parses_set_names_from_noisy_ocr() {
        assert_eq!(
            parse_set_name("Sun-sinking Eclipse"),
            Some(EchoSet::SunSinkingEclipse)
        );
        assert_eq!(
            parse_set_name("MOLTEN RIFT"),
            Some(EchoSet::MoltenRiftEmbers)
        );
        assert_eq!(
            parse_set_name("2-Set: Lingering Tunes"),
            Some(EchoSet::LingeringTunes)
        );
        assert_eq!(parse_set_name("unrelated text"), None);
    }

    // --- test fixtures ------------------------------------------------------

    fn echo(id: u32, cost: u8, set: EchoSet, main: (Stat, f32), subs: &[(Stat, f32)]) -> Echo {
        Echo {
            id,
            name: format!("echo-{id}"),
            set,
            cost,
            level: 25,
            main_stat: StatRoll::new(main.0, main.1),
            substats: subs.iter().map(|(s, v)| StatRoll::new(*s, *v)).collect(),
        }
    }

    /// A small but non-trivial inventory: enough distinct echoes per slot that
    /// the optimizer has real choices to make.
    fn sample_inventory() -> Inventory {
        use EchoSet::*;
        use Stat::*;
        Inventory::new(vec![
            // cost-4 candidates
            echo(
                1,
                4,
                SunSinkingEclipse,
                (CritDmg, 44.0),
                &[(CritRate, 9.0), (AtkPct, 8.0)],
            ),
            echo(
                2,
                4,
                SunSinkingEclipse,
                (CritRate, 22.0),
                &[(CritDmg, 12.0), (Atk, 40.0)],
            ),
            echo(3, 4, MoltenRiftEmbers, (AtkPct, 33.0), &[(CritRate, 6.0)]),
            // cost-3 candidates
            echo(
                10,
                3,
                SunSinkingEclipse,
                (Fusion, 30.0),
                &[(CritRate, 10.5), (CritDmg, 21.0)],
            ),
            echo(11, 3, SunSinkingEclipse, (AtkPct, 30.0), &[(CritDmg, 14.0)]),
            echo(12, 3, SunSinkingEclipse, (Fusion, 30.0), &[(AtkPct, 9.0)]),
            echo(13, 3, MoltenRiftEmbers, (Fusion, 30.0), &[(CritRate, 5.0)]),
            // cost-1 candidates
            echo(
                20,
                1,
                SunSinkingEclipse,
                (AtkPct, 18.0),
                &[(CritRate, 9.0), (CritDmg, 18.0)],
            ),
            echo(21, 1, SunSinkingEclipse, (AtkPct, 18.0), &[(Atk, 30.0)]),
            echo(22, 1, SunSinkingEclipse, (Hp, 2280.0), &[(HpPct, 8.0)]),
            echo(23, 1, MoltenRiftEmbers, (AtkPct, 18.0), &[(CritDmg, 12.0)]),
        ])
    }

    // --- scoring ------------------------------------------------------------

    #[test]
    fn evaluator_rewards_crit() {
        let inv = sample_inventory();
        let eval = WeightedSubstatEvaluator::new(CharacterProfile::generic_dps());
        // A build stacked with crit should outscore one without.
        let crit_build = Build {
            slots: [
                inv.get(2).unwrap(),
                inv.get(10).unwrap(),
                inv.get(11).unwrap(),
                inv.get(20).unwrap(),
                inv.get(23).unwrap(),
            ],
        };
        let dull_build = Build {
            slots: [
                inv.get(3).unwrap(),
                inv.get(12).unwrap(),
                inv.get(13).unwrap(),
                inv.get(21).unwrap(),
                inv.get(22).unwrap(),
            ],
        };
        assert!(eval.score(&crit_build) > eval.score(&dull_build));
    }

    // --- optimizer ----------------------------------------------------------

    #[test]
    fn ga_matches_exhaustive_optimum() {
        // The strongest correctness check we can make: on a search space small
        // enough to brute-force, the genetic algorithm must find a build that
        // scores exactly as well as the provable optimum.
        let inv = sample_inventory();
        let eval = WeightedSubstatEvaluator::new(CharacterProfile::generic_dps());
        let spec = BuildSpec::default();

        let truth = optimize_exhaustive(&inv, &spec, &eval, 10_000_000).unwrap();
        let ga = optimize_ga(&inv, &spec, &eval, &GaConfig::default()).unwrap();

        assert_eq!(truth.method, "exhaustive");
        assert_eq!(ga.method, "genetic");
        // Scores must match; the exact echo ids may differ if there are ties.
        assert!(
            (truth.score - ga.score).abs() < 1e-4,
            "GA {} did not reach optimum {}",
            ga.score,
            truth.score
        );
    }

    #[test]
    fn optimizer_never_reuses_an_echo() {
        let inv = sample_inventory();
        let eval = WeightedSubstatEvaluator::new(CharacterProfile::generic_dps());
        let result = optimize_ga(&inv, &BuildSpec::default(), &eval, &GaConfig::default()).unwrap();
        let mut ids = result.echo_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 5, "an echo was placed in two slots");
    }

    #[test]
    fn required_set_filters_candidates() {
        let inv = sample_inventory();
        let eval = WeightedSubstatEvaluator::new(CharacterProfile::generic_dps());
        let spec = BuildSpec {
            required_set: Some(EchoSet::SunSinkingEclipse),
            ..Default::default()
        };
        let result = optimize_exhaustive(&inv, &spec, &eval, 10_000_000).unwrap();
        // Every returned echo must belong to the required set.
        for id in result.echo_ids {
            assert_eq!(inv.get(id).unwrap().set, EchoSet::SunSinkingEclipse);
        }
    }

    #[test]
    fn empty_slot_is_a_clear_error() {
        // Inventory with no 4-cost echo.
        let inv = Inventory::new(vec![
            echo(10, 3, EchoSet::LingeringTunes, (Stat::AtkPct, 30.0), &[]),
            echo(11, 3, EchoSet::LingeringTunes, (Stat::AtkPct, 30.0), &[]),
            echo(20, 1, EchoSet::LingeringTunes, (Stat::AtkPct, 18.0), &[]),
            echo(21, 1, EchoSet::LingeringTunes, (Stat::AtkPct, 18.0), &[]),
        ]);
        let eval = WeightedSubstatEvaluator::new(CharacterProfile::generic_dps());
        let err =
            optimize_ga(&inv, &BuildSpec::default(), &eval, &GaConfig::default()).unwrap_err();
        assert_eq!(err, OptimizeError::EmptySlot(SlotGroup::Cost4));
    }

    // --- serialization ------------------------------------------------------

    #[test]
    fn inventory_json_round_trips() {
        let inv = sample_inventory();
        let json = serde_json::to_string(&inv).unwrap();
        let back: Inventory = serde_json::from_str(&json).unwrap();
        assert_eq!(inv.len(), back.len());
        assert_eq!(inv.echoes[0], back.echoes[0]);
    }
}