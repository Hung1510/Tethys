//! Writes the built-in character profiles to assets/profiles as JSON references.
//! Run with: cargo run -p tethys-core --example dump_profiles
use tethys_core::score::CharacterProfile;

fn main() {
    for (file, profile) in [
        (
            "assets/profiles/generic_dps.json",
            CharacterProfile::generic_dps(),
        ),
        (
            "assets/profiles/support_er.json",
            CharacterProfile::support_er(),
        ),
    ] {
        let json = serde_json::to_string_pretty(&profile).unwrap();
        std::fs::write(file, json).unwrap();
        println!("wrote {file}");
    }
}
