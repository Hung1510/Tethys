//! Tethys command-line entry point.
//!
//! Subcommands:
//!   tethys sample                       print a sample inventory (the JSON schema)
//!   tethys optimize <inv.json> [flags]  optimize a build from an inventory file
//!   tethys calibrate [out.png]          save a capture-region overlay (needs `capture`)
//!
//! optimize flags:
//!   --profile <dps|support>   character weighting to optimize for (default: dps)
//!   --set <SetName>           require a five-piece of this echo set
//!   --method <ga|exhaustive>  solver (default: ga)
//!
//! The GUI (feature `gui`) is a separate front-end over the same core.

mod sample;

use anyhow::{bail, Context, Result};
use std::env;
use tethys_core::model::{Echo, Inventory};
use tethys_core::prelude::*;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => default_action(),
        Some("sample") => {
            let inv = sample::sample_inventory();
            println!("{}", serde_json::to_string_pretty(&inv)?);
            Ok(())
        }
        Some("optimize") => cmd_optimize(&args[1..]),
        Some("calibrate") => cmd_calibrate(&args[1..]),
        Some("scan") => cmd_scan(),
        Some("windows") => cmd_list_windows(),
        Some("--gui") | Some("gui") => launch_gui(),
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn cmd_scan() -> Result<()> {
    #[cfg(all(feature = "capture", feature = "windows-ocr"))]
    {
        let engine = tethys_scanner::ocr::WindowsOcr::new()
            .with_context(|| "starting the Windows OCR engine")?;
        let image = tethys_scanner::capture::capture_window_image()?;
        let stats = tethys_scanner::scan_echo_panel(
            &engine,
            &image,
            &tethys_scanner::EchoDetailLayout::default_16_9(),
        )?;

        if stats.is_empty() {
            println!(
                "Read no stats. Open an echo's detail panel in-game, then check the \
                 regions line up with `tethys calibrate cal.png`."
            );
        } else {
            println!("Read {} stat(s) from the open echo panel:", stats.len());
            for s in &stats {
                let pct = if s.stat.is_percent() { "%" } else { "" };
                println!("  {:?}  {:.1}{pct}", s.stat, s.value);
            }
        }
        return Ok(());
    }

    #[cfg(not(all(feature = "capture", feature = "windows-ocr")))]
    {
        anyhow::bail!(
            "scan needs the `capture` and `windows-ocr` features; \
             build with --features capture,windows-ocr"
        )
    }
}

fn cmd_list_windows() -> Result<()> {
    let windows = tethys_scanner::capture::list_windows()?;
    if windows.is_empty() {
        println!("No windows enumerated. Try running this terminal as Administrator.");
        return Ok(());
    }
    println!("Enumerable windows ({}):", windows.len());
    for (title, minimized) in &windows {
        let flag = if *minimized { "min " } else { "show" };
        let shown = if title.is_empty() {
            "<no title>"
        } else {
            title.as_str()
        };
        println!("  [{flag}] {shown}");
    }
    println!(
        "\nFind the Wuthering Waves entry above. If its title differs from\n\
         \"Wuthering Waves\", update GAME_WINDOW_TITLE in crates/scanner/src/capture.rs\n\
         to a lowercase substring of the real title."
    );
    Ok(())
}

fn cmd_calibrate(args: &[String]) -> Result<()> {
    // Optional output path; defaults to tethys-calibration.png.
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "tethys-calibration.png".to_string());
    let include_grid = args.iter().any(|a| a == "--grid");

    let panel = tethys_scanner::EchoDetailLayout::default_16_9();
    let grid = tethys_scanner::GridLayout::default_16_9();
    let grid_ref = if include_grid { Some(&grid) } else { None };

    tethys_scanner::save_calibration(&out, &panel, grid_ref)
        .with_context(|| "saving calibration overlay")?;

    println!("Wrote calibration overlay to {out}");
    println!(
        "Detail-panel boxes: red = name, amber = cost, green = main stat, cyan = substats.\n\
         {}\
         If anything is off, tune the layouts in crates/scanner/src/layout.rs.",
        if include_grid {
            "Magenta boxes = inventory grid cells.\n"
        } else {
            "Pass --grid to also overlay the inventory grid.\n"
        }
    );
    Ok(())
}

fn cmd_optimize(args: &[String]) -> Result<()> {
    let mut path: Option<String> = None;
    let mut profile_name = "dps".to_string();
    let mut method = "ga".to_string();
    let mut set: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--profile" => {
                profile_name = take(args, &mut i, "--profile")?;
            }
            "--method" => {
                method = take(args, &mut i, "--method")?;
            }
            "--set" => {
                set = Some(take(args, &mut i, "--set")?);
            }
            other if !other.starts_with("--") => path = Some(other.to_string()),
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }

    let inv: Inventory = match path {
        Some(p) => {
            let text = std::fs::read_to_string(&p)
                .with_context(|| format!("reading inventory file {p}"))?;
            serde_json::from_str(&text).with_context(|| "parsing inventory JSON")?
        }
        None => {
            eprintln!("no inventory file given; using the built-in sample\n");
            sample::sample_inventory()
        }
    };

    let profile = match profile_name.as_str() {
        "dps" => CharacterProfile::generic_dps(),
        "support" => CharacterProfile::support_er(),
        other => bail!("unknown profile '{other}' (expected dps or support)"),
    };
    let eval = WeightedSubstatEvaluator::new(profile);

    let spec = BuildSpec {
        required_set: set.as_deref().map(parse_set).transpose()?,
        ..Default::default()
    };

    let result = match method.as_str() {
        "ga" => optimize_ga(&inv, &spec, &eval, &GaConfig::default())?,
        "exhaustive" => optimize_exhaustive(&inv, &spec, &eval, 50_000_000)?,
        other => bail!("unknown method '{other}' (expected ga or exhaustive)"),
    };

    print_result(&inv, &result);
    Ok(())
}

fn print_result(inv: &Inventory, result: &OptimizeResult) {
    let slot_names = ["4-cost", "3-cost", "3-cost", "1-cost", "1-cost"];
    println!("Recommended build ({} solver)", result.method);
    println!("  score:       {:.3}", result.score);
    println!("  evaluations: {}", result.evaluations);
    println!();
    for (slot, id) in slot_names.iter().zip(result.echo_ids.iter()) {
        if let Some(e) = inv.get(*id) {
            println!("  [{slot}] {}", describe_echo(e));
        }
    }
}

fn describe_echo(e: &Echo) -> String {
    let subs: Vec<String> = e
        .substats
        .iter()
        .map(|s| format!("{:?} {}", s.stat, fmt_val(s.value, s.stat.is_percent())))
        .collect();
    format!(
        "{:?} | main: {:?} {} | {}",
        e.set,
        e.main_stat.stat,
        fmt_val(e.main_stat.value, e.main_stat.stat.is_percent()),
        if subs.is_empty() {
            "(no substats)".to_string()
        } else {
            subs.join(", ")
        }
    )
}

fn fmt_val(v: f32, pct: bool) -> String {
    if pct {
        format!("{v:.1}%")
    } else {
        format!("{v:.0}")
    }
}

fn parse_set(name: &str) -> Result<EchoSet> {
    use EchoSet::*;
    let normalized = name.to_lowercase().replace([' ', '-', '_'], "");
    let set = match normalized.as_str() {
        "freezingfrost" => FreezingFrost,
        "moltenriftembers" | "moltenrift" => MoltenRiftEmbers,
        "voidthunder" => VoidThunder,
        "sierragale" => SierraGale,
        "celestiallight" => CelestialLight,
        "sunsinkingeclipse" | "sinkingeclipse" => SunSinkingEclipse,
        "rejuvenatingglow" => RejuvenatingGlow,
        "moonlitclouds" => MoonlitClouds,
        "lingeringtunes" => LingeringTunes,
        "frostyresolve" => FrostyResolve,
        "eternalradiance" => EternalRadiance,
        "midnightveil" => MidnightVeil,
        "empyreananthem" => EmpyreanAnthem,
        "tidebreakingcourage" => TidebreakingCourage,
        other => bail!("unknown set '{other}'"),
    };
    Ok(set)
}

/// Advance `i` past a flag's value and return it.
fn take(args: &[String], i: &mut usize, flag: &str) -> Result<String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .with_context(|| format!("{flag} needs a value"))
}

#[cfg(feature = "gui")]
fn launch_gui() -> Result<()> {
    tethys_app_gui::launch()
}

#[cfg(not(feature = "gui"))]
fn launch_gui() -> Result<()> {
    anyhow::bail!("this build has no GUI; rebuild with `--features gui`")
}

/// What happens when the binary is run with no arguments (e.g. double-clicked):
/// open the GUI if this build has one, otherwise print usage.
#[cfg(feature = "gui")]
fn default_action() -> Result<()> {
    launch_gui()
}

#[cfg(not(feature = "gui"))]
fn default_action() -> Result<()> {
    print_usage();
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Tethys — Wuthering Waves echo optimizer\n\n\
         USAGE:\n\
         \x20 tethys sample                       print a sample inventory (JSON schema)\n\
         \x20 tethys optimize <inv.json> [flags]  optimize a build\n\
         \x20 tethys calibrate [out.png] [--grid]  save a capture-region overlay (needs `capture`)\n\
         \x20 tethys scan                          read the open echo panel (needs `capture,windows-ocr`)\n\
         \x20 tethys windows                        list window titles for scan setup (needs `capture`)\n\n\
         OPTIMIZE FLAGS:\n\
         \x20 --profile <dps|support>   weighting to optimize for (default: dps)\n\
         \x20 --set <SetName>           require a five-piece of this set\n\
         \x20 --method <ga|exhaustive>  solver (default: ga)\n"
    );
}

// The GUI module is compiled only with the `gui` feature.
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
use gui as tethys_app_gui;