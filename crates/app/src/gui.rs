//! Desktop GUI front-end (feature `gui`).
//!
//! This is the seed of the "real product": a window where you scan echoes,
//! pick a character, and see the optimized build. Version 0.1 wires the
//! optimizer to a minimal egui window over the built-in sample inventory; the
//! scan button and a Shorekeeper-themed skin are the next milestones.
//!
//! Built against eframe 0.27. Compiled only with `--features gui`.

use crate::sample::sample_inventory;
use anyhow::Result;
use eframe::egui;
use tethys_core::model::Inventory;
use tethys_core::prelude::*;

pub fn launch() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tethys — Echo Optimizer",
        options,
        Box::new(|_cc| Box::new(TethysApp::default())),
    )
    .map_err(|e| anyhow::anyhow!("failed to launch GUI: {e}"))
}

struct TethysApp {
    inventory: Inventory,
    profile: ProfileChoice,
    result: Option<OptimizeResult>,
    status: String,
}

#[derive(PartialEq, Clone, Copy)]
enum ProfileChoice {
    Dps,
    Support,
}

impl Default for TethysApp {
    fn default() -> Self {
        Self {
            inventory: sample_inventory(),
            profile: ProfileChoice::Dps,
            result: None,
            status: "Loaded built-in sample inventory. Scan integration is next.".into(),
        }
    }
}

impl TethysApp {
    fn optimize(&mut self) {
        let profile = match self.profile {
            ProfileChoice::Dps => CharacterProfile::generic_dps(),
            ProfileChoice::Support => CharacterProfile::support_er(),
        };
        let eval = WeightedSubstatEvaluator::new(profile);
        match optimize_ga(
            &self.inventory,
            &BuildSpec::default(),
            &eval,
            &GaConfig::default(),
        ) {
            Ok(r) => {
                self.status = format!("Optimized in {} evaluations.", r.evaluations);
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

            ui.horizontal(|ui| {
                ui.label("Character profile:");
                ui.selectable_value(&mut self.profile, ProfileChoice::Dps, "Generic DPS");
                ui.selectable_value(&mut self.profile, ProfileChoice::Support, "Support (ER)");
            });

            ui.add_space(8.0);
            if ui.button("Optimize").clicked() {
                self.optimize();
            }

            ui.add_space(8.0);
            ui.label(&self.status);
            ui.separator();

            if let Some(result) = &self.result {
                ui.label(format!("Score: {:.3}", result.score));
                let slot_names = ["4-cost", "3-cost", "3-cost", "1-cost", "1-cost"];
                for (slot, id) in slot_names.iter().zip(result.echo_ids.iter()) {
                    if let Some(e) = self.inventory.get(*id) {
                        ui.label(format!("[{slot}] {} — {:?}", e.name, e.set));
                    }
                }
            }
        });
    }
}
