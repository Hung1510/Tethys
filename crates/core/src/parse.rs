//! Parsing OCR output into structured stats.
//!
//! The scanner crate is responsible for turning pixels into text lines; this
//! module turns those text lines into [`StatRoll`]s. It is deliberately pure
//! and dependency-free so every messy real-world OCR case can be covered by a
//! fast unit test rather than a screenshot fixture.

use crate::model::{Stat, StatRoll};

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
