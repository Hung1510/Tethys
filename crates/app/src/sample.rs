//! A built-in sample inventory so `tethys optimize` works with zero setup and
//! `tethys sample` documents the JSON schema users export their echoes into.

use tethys_core::model::{Echo, EchoSet, Inventory, Stat, StatRoll};

fn echo(
    id: u32,
    name: &str,
    cost: u8,
    set: EchoSet,
    main: (Stat, f32),
    subs: &[(Stat, f32)],
) -> Echo {
    Echo {
        id,
        name: name.to_string(),
        set,
        cost,
        level: 25,
        main_stat: StatRoll::new(main.0, main.1),
        substats: subs.iter().map(|(s, v)| StatRoll::new(*s, *v)).collect(),
    }
}

pub fn sample_inventory() -> Inventory {
    use EchoSet::*;
    use Stat::*;
    Inventory::new(vec![
        echo(
            1,
            "Lampylumen Myriad",
            4,
            SunSinkingEclipse,
            (CritDmg, 44.0),
            &[(CritRate, 9.0), (AtkPct, 8.6)],
        ),
        echo(
            2,
            "Mourning Aix",
            4,
            SunSinkingEclipse,
            (CritRate, 22.0),
            &[(CritDmg, 15.0), (Atk, 40.0)],
        ),
        echo(
            3,
            "Inferno Rider",
            4,
            MoltenRiftEmbers,
            (AtkPct, 33.0),
            &[(CritRate, 6.3)],
        ),
        echo(
            10,
            "Hoochief",
            3,
            SunSinkingEclipse,
            (Fusion, 30.0),
            &[(CritRate, 10.5), (CritDmg, 21.0)],
        ),
        echo(
            11,
            "Tambourinist",
            3,
            SunSinkingEclipse,
            (AtkPct, 30.0),
            &[(CritDmg, 14.7)],
        ),
        echo(
            12,
            "Fusion Warrior",
            3,
            SunSinkingEclipse,
            (Fusion, 30.0),
            &[(AtkPct, 9.4)],
        ),
        echo(
            13,
            "Lava Larva",
            3,
            MoltenRiftEmbers,
            (Fusion, 30.0),
            &[(CritRate, 5.1)],
        ),
        echo(
            20,
            "Fission Junrock",
            1,
            SunSinkingEclipse,
            (AtkPct, 18.0),
            &[(CritRate, 9.0), (CritDmg, 18.0)],
        ),
        echo(
            21,
            "Vanguard Junrock",
            1,
            SunSinkingEclipse,
            (AtkPct, 18.0),
            &[(Atk, 30.0)],
        ),
        echo(
            22,
            "Whiff Whaff",
            1,
            SunSinkingEclipse,
            (HpPct, 22.8),
            &[(HpPct, 8.0)],
        ),
        echo(
            23,
            "Excarat",
            1,
            MoltenRiftEmbers,
            (AtkPct, 18.0),
            &[(CritDmg, 12.6)],
        ),
    ])
}
