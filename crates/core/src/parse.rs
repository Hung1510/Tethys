//! Parsing OCR output into structured stats.
//!
//! The scanner crate is responsible for turning pixels into text lines; this
//! module turns those text lines into [`StatRoll`]s. It is deliberately pure
//! and dependency-free so every messy real-world OCR case can be covered by a
//! fast unit test rather than a screenshot fixture.

use crate::model::{EchoSet, Stat, StatRoll};

/// Parse a single OCR line such as `"Crit. Rate 6.3%"` or `"ATK 40"` into a
/// [`StatRoll`]. Returns `None` for lines that do not describe a stat.
///
/// Percentage vs. flat is disambiguated by the presence of `%` in the line,
/// which matters for the ATK / HP / DEF stats that exist in both forms.
pub fn parse_substat_line(line: &str) -> Option<StatRoll> {
    let (value, num_start) = extract_number(line)?;
    let label = normalize_label(&line[..num_start]);
    let is_percent = line.contains('%');
    let stat = match_stat(&label, is_percent)?;
    Some(StatRoll::new(stat, value))
}

/// Parse many lines, keeping only the ones that describe a stat. Useful for a
/// whole echo panel dumped as text.
pub fn parse_lines<'a, I>(lines: I) -> Vec<StatRoll>
where
    I: IntoIterator<Item = &'a str>,
{
    lines.into_iter().filter_map(parse_substat_line).collect()
}

/// Find the first numeric token (integer or decimal) and return its value plus
/// the byte offset where it begins, so the caller can slice off the label.
fn extract_number(s: &str) -> Option<(f32, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut end = i;
            while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
                end += 1;
            }
            // Trim a trailing '.' that belonged to a label, not the number.
            let mut slice = &s[start..end];
            if slice.ends_with('.') {
                slice = &slice[..slice.len() - 1];
            }
            if let Ok(v) = slice.parse::<f32>() {
                return Some((v, start));
            }
        }
        i += 1;
    }
    None
}

/// Lowercase, drop punctuation and the percent sign, collapse whitespace.
fn normalize_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        if ch.is_alphabetic() {
            out.push(ch.to_ascii_lowercase());
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        }
        // periods, %, digits, etc. are dropped
    }
    out.trim().to_string()
}

/// Map a normalized label (+ percent flag) to a [`Stat`].
fn match_stat(label: &str, is_percent: bool) -> Option<Stat> {
    // Longest / most specific labels first.
    let stat = match label {
        "crit rate" | "critical rate" | "crit rat" => Stat::CritRate,
        "crit dmg" | "critical dmg" | "crit damage" | "critical damage" => Stat::CritDmg,
        "energy regen" | "energy regeneration" | "er" => Stat::EnergyRegen,
        "healing bonus" | "healing" => Stat::HealingBonus,
        "basic attack dmg bonus" | "basic attack" | "basic attack dmg" => Stat::BasicAtk,
        "heavy attack dmg bonus" | "heavy attack" | "heavy attack dmg" => Stat::HeavyAtk,
        "resonance skill dmg bonus" | "resonance skill" | "resonance skill dmg" => {
            Stat::ResonanceSkill
        }
        "resonance liberation dmg bonus" | "resonance liberation" | "resonance liberation dmg" => {
            Stat::ResonanceLiberation
        }
        "glacio dmg bonus" | "glacio dmg" | "glacio" => Stat::Glacio,
        "fusion dmg bonus" | "fusion dmg" | "fusion" => Stat::Fusion,
        "electro dmg bonus" | "electro dmg" | "electro" => Stat::Electro,
        "aero dmg bonus" | "aero dmg" | "aero" => Stat::Aero,
        "spectro dmg bonus" | "spectro dmg" | "spectro" => Stat::Spectro,
        "havoc dmg bonus" | "havoc dmg" | "havoc" => Stat::Havoc,
        // Ambiguous flat/percent stats resolved by the percent flag.
        "atk" | "attack" => return Some(if is_percent { Stat::AtkPct } else { Stat::Atk }),
        "hp" => return Some(if is_percent { Stat::HpPct } else { Stat::Hp }),
        "def" | "defense" | "defence" => {
            return Some(if is_percent { Stat::DefPct } else { Stat::Def })
        }
        _ => return None,
    };
    Some(stat)
}

/// Parse an echo cost (1, 3, or 4) from OCR text like `"COST 4"` or `"4"`.
/// Only 1/3/4 are valid echo costs, so stray numbers (e.g. a `44.0%` main
/// stat) are ignored.
pub fn parse_cost(text: &str) -> Option<u8> {
    for token in text.split(|c: char| !c.is_ascii_digit()) {
        if let Ok(n) = token.parse::<u8>() {
            if matches!(n, 1 | 3 | 4) {
                return Some(n);
            }
        }
    }
    None
}

/// Infer an echo's cost from its main stat, for the common case where the main
/// stat only appears at one cost. Crit / healing mains are 4-cost; elemental
/// and energy-regen mains are 3-cost; flat HP/ATK/DEF mains are 1-cost. The
/// percentage mains (HP%/ATK%/DEF%) appear at several costs and return `None`
/// — those need the numeric cost from [`parse_cost`].
pub fn infer_cost_from_main(stat: Stat) -> Option<u8> {
    match stat {
        Stat::CritRate | Stat::CritDmg | Stat::HealingBonus => Some(4),
        Stat::Glacio
        | Stat::Fusion
        | Stat::Electro
        | Stat::Aero
        | Stat::Spectro
        | Stat::Havoc
        | Stat::EnergyRegen => Some(3),
        Stat::Hp | Stat::Atk | Stat::Def => Some(1),
        _ => None,
    }
}

/// Match OCR text of the sonata (set) name against the known echo sets. The
/// text is normalised to lowercase letters, so casing, spacing, and hyphens in
/// the OCR output don't matter.
pub fn parse_set_name(text: &str) -> Option<EchoSet> {
    let norm = normalize_alpha(text);
    // Most specific keys first so, e.g., "sinkingeclipse" still resolves.
    const SETS: &[(&str, EchoSet)] = &[
        ("freezingfrost", EchoSet::FreezingFrost),
        ("moltenrift", EchoSet::MoltenRiftEmbers),
        ("voidthunder", EchoSet::VoidThunder),
        ("sierragale", EchoSet::SierraGale),
        ("celestiallight", EchoSet::CelestialLight),
        ("sunsinkingeclipse", EchoSet::SunSinkingEclipse),
        ("sinkingeclipse", EchoSet::SunSinkingEclipse),
        ("rejuvenatingglow", EchoSet::RejuvenatingGlow),
        ("moonlitclouds", EchoSet::MoonlitClouds),
        ("lingeringtunes", EchoSet::LingeringTunes),
        ("frostyresolve", EchoSet::FrostyResolve),
        ("eternalradiance", EchoSet::EternalRadiance),
        ("midnightveil", EchoSet::MidnightVeil),
        ("empyreananthem", EchoSet::EmpyreanAnthem),
        ("tidebreakingcourage", EchoSet::TidebreakingCourage),
    ];
    SETS.iter()
        .find(|(key, _)| norm.contains(key))
        .map(|(_, set)| *set)
}

/// Lowercase, letters only (drops spaces, digits, punctuation).
fn normalize_alpha(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}