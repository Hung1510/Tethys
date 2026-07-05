//! Desktop GUI front-end (feature `gui`).
//!
//! A window over the same core the CLI uses: load an inventory (the built-in
//! sample or a JSON file exported/scanned elsewhere), pick a character profile
//! and solver, optimize, and read the recommended build.
//!
//! Scanning is not yet wired into the GUI — it waits on a verified OCR backend
//! (the Windows OCR API is the next milestone). For now the CLI's `calibrate`
//! handles the capture side. Built against eframe 0.27.

use crate::sample::sample_inventory;
use anyhow::Result;
use eframe::egui;
use tethys_core::model::{Echo, Inventory};
use tethys_core::prelude::*;

pub fn launch() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([560.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tethys — Echo Optimizer",
        options,
        Box::new(|_cc| Box::new(TethysApp::default())),
    )
    .map_err(|e| anyhow::anyhow!("failed to launch GUI: {e}"))
}

#[derive(PartialEq, Clone, Copy)]
enum ProfileChoice {
    Dps,
    Support,
}

#[derive(PartialEq, Clone, Copy)]
enum MethodChoice {
    Genetic,
    Exhaustive,
}

struct TethysApp {
    inventory: Inventory,
    inventory_source: String,
    path_input: String,
    profile: ProfileChoice,
    method: MethodChoice,
    result: Option<OptimizeResult>,
    status: String,
}

impl Default for TethysApp {
    fn default() -> Self {
        let inventory = sample_inventory();
        let n = inventory.len();
        Self {
            inventory,
            inventory_source: format!("built-in sample ({n} echoes)"),
            path_input: String::new(),
            profile: ProfileChoice::Dps,
            method: MethodChoice::Genetic,
            result: None,
            status: "Loaded the built-in sample inventory. Optimize, or load your own JSON."
                .to_string(),
        }
    }
}

impl TethysApp {
    fn load_json(&mut self, path: &str) {
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Inventory>(&t).map_err(|e| e.to_string()))
        {
            Ok(inv) => {
                let n = inv.len();
                self.inventory = inv;
                self.inventory_source = format!("{path} ({n} echoes)");
                self.result = None;
                self.status = format!("Loaded {n} echoes from {path}.");
            }
            Err(e) => {
                self.status = format!("Could not load {path}: {e}");
            }
        }
    }

    fn optimize(&mut self) {
        let profile = match self.profile {
            ProfileChoice::Dps => CharacterProfile::generic_dps(),
            ProfileChoice::Support => CharacterProfile::support_er(),
        };
        let eval = WeightedSubstatEvaluator::new(profile);
        let spec = BuildSpec::default();

        let outcome = match self.method {
            MethodChoice::Genetic => {
                optimize_ga(&self.inventory, &spec, &eval, &GaConfig::default())
            }
            MethodChoice::Exhaustive => {
                optimize_exhaustive(&self.inventory, &spec, &eval, 50_000_000)
            }
        };

        match outcome {
            Ok(r) => {
                self.status = format!(
                    "Optimized ({} solver, {} builds scored).",
                    r.method, r.evaluations
                );
                self.result = Some(r);
            }
            Err(e) => {
                self.status = format!("Could not optimize: {e}");
                self.result = None;
            }
        }
    }
}

impl eframe::App for TethysApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Tethys");
            ui.label("Wuthering Waves echo optimizer");
            ui.separator();

            // Inventory source.
            ui.horizontal(|ui| {
                ui.label("Inventory:");
                ui.monospace(&self.inventory_source);
            });
            ui.horizontal(|ui| {
                if ui.button("Use sample").clicked() {
                    *self = TethysApp::default();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("path to inventory .json")
                        .desired_width(320.0),
                );
                if ui.button("Load JSON").clicked() {
                    let path = self.path_input.trim().to_string();
                    if !path.is_empty() {
                        self.load_json(&path);
                    }
                }
            });

            ui.add_space(6.0);
            ui.separator();

            // Options.
            ui.horizontal(|ui| {
                ui.label("Profile:");
                ui.selectable_value(&mut self.profile, ProfileChoice::Dps, "Generic DPS");
                ui.selectable_value(&mut self.profile, ProfileChoice::Support, "Support (ER)");
            });
            ui.horizontal(|ui| {
                ui.label("Solver:");
                ui.selectable_value(&mut self.method, MethodChoice::Genetic, "Genetic");
                ui.selectable_value(&mut self.method, MethodChoice::Exhaustive, "Exhaustive");
            });

            ui.add_space(8.0);
            if ui.button("  Optimize  ").clicked() {
                self.optimize();
            }

            ui.add_space(6.0);
            ui.label(&self.status);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(result) = &self.result {
                    ui.strong(format!("Recommended build — score {:.3}", result.score));
                    ui.add_space(4.0);
                    let slot_names = ["4-cost", "3-cost", "3-cost", "1-cost", "1-cost"];
                    for (slot, id) in slot_names.iter().zip(result.echo_ids.iter()) {
                        if let Some(e) = self.inventory.get(*id) {
                            ui.label(format!("[{slot}]  {}", describe_echo(e)));
                        }
                    }
                }
            });
        });
    }
}

fn describe_echo(e: &Echo) -> String {
    let subs: Vec<String> = e
        .substats
        .iter()
        .map(|s| format!("{:?} {:.1}", s.stat, s.value))
        .collect();
    format!(
        "{:?} | {:?} {:.1} | {}",
        e.set,
        e.main_stat.stat,
        e.main_stat.value,
        if subs.is_empty() {
            "(no substats)".to_string()
        } else {
            subs.join(", ")
        }
    )
}