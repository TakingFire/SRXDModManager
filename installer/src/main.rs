#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod gui;
mod patch;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "en");

use crate::app::Installer;
use crate::gui::Gui;

fn main() -> eframe::Result {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut installer = Installer::default();
    installer.init();

    let mut gui = Gui::default();
    gui.installer = installer;

    let app = Box::new(gui);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(eframe::egui::IconData {
                rgba: include_bytes!("./assets/icon.rgba").to_vec(),
                width: 64,
                height: 64,
            })
            .with_inner_size([600.0, 400.0])
            .with_resizable(true)
            .with_active(true),
        ..Default::default()
    };

    eframe::run_native("SRXD Mod Manager", options, Box::new(|_| Ok(app)))
}
