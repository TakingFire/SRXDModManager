use std::{collections::HashSet, time::Duration};

use eframe::egui::{
    self, CentralPanel, Color32, ComboBox, Context, Frame, Id, Modal, OpenUrl, RichText,
    ScrollArea, SidePanel, TextEdit, TopBottomPanel,
};

use crate::app::{Installer, InstallerState, ModEntry, ModEntryRef, ModEntryState};

#[allow(unused)]
const GUIDE_URL: &str = "https://useredge.github.io/spinshare-wiki/modding/installation-guide/";
const UPDATE_URL: &str = "https://github.com/TakingFire/SRXDModManager/releases/latest";
const ISSUES_URL: &str = "https://github.com/TakingFire/SRXDModManager/issues/new";

#[derive(Default)]
pub struct Gui {
    pub installer: Installer,

    categories: HashSet<String>,
    filter_by: FilterBy,
    sort_by: SortBy,
    search: String,

    filtered_mods: Vec<ModEntryRef>,
    updatable_mods: Vec<ModEntryRef>,

    show_disclaimer: bool,
    disclaimer_checkbox: bool,
    show_linux_guide: bool,
    #[allow(unused)]
    linux_guide_checkbox: bool,

    initialized: bool,
    show_debug: bool,
}

#[derive(Debug, Default, PartialEq, Copy, Clone)]
enum FilterBy {
    #[default]
    All,
    Installed,
    Uninstalled,
    Updatable,
}

#[derive(Debug, Default, PartialEq, Copy, Clone)]
enum SortBy {
    Recent,
    #[default]
    Title,
    Author,
}

impl eframe::App for Gui {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}

    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        ctx.input(|input| {
            if input.key_pressed(egui::Key::F1) {
                self.show_debug = !self.show_debug;
            }
        });

        if !self.initialized {
            ctx.set_fonts(load_font());
            if let Some(storage) = frame.storage() {
                self.show_disclaimer = storage.get_string("show_disclaimer").is_none();
                #[cfg(not(target_os = "windows"))]
                {
                    self.show_linux_guide = storage.get_string("show_linux_guide").is_none();
                }
            }
            self.build_list();
            self.initialized = true;
        };

        self.installer.update();

        if self.installer.force_ui_update {
            self.build_list();
            ctx.request_repaint();
            self.installer.force_ui_update = false;
        }

        if matches!(self.installer.state, InstallerState::Outdated) {
            self.draw_outdated_warning(ctx);
        } else {
            #[cfg(not(target_os = "windows"))]
            if self.show_linux_guide {
                self.draw_linux_guide(ctx, frame);
            }

            if self.show_disclaimer && !self.show_linux_guide {
                self.draw_disclaimer(ctx, frame);
            }
        }

        if matches!(self.installer.state, InstallerState::Error) {
            self.draw_error_bar(ctx);
        }

        self.draw_sidebar(ctx);

        if !self.updatable_mods.is_empty() {
            self.draw_update_bar(ctx);
        }

        self.draw_mod_list(ctx);

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl Gui {
    fn draw_outdated_warning(&mut self, ctx: &Context) {
        Modal::new(Id::new("ui_disclaimer")).show(ctx, |ui| {
            ui.set_width(240.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Update Available").size(18.0));
                ui.label(
                    "This version may no longer be compatible. Please get the latest version here:",
                );
                ui.hyperlink_to("Download Page", UPDATE_URL);

                ui.add_space(8.0);
                if ui.button("Try Anyway").clicked() {
                    self.installer.state = InstallerState::Init;
                    self.installer.get_patcher();
                }
            });
        });
    }

    fn draw_disclaimer(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        Modal::new(Id::new("ui_disclaimer")).show(ctx, |ui| {
            ui.set_width(220.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Warning").size(18.0));
                ui.label("Mod releases are not actively verified. Download at your own risk.");

                ui.add_space(8.0);
                ui.checkbox(&mut self.disclaimer_checkbox, "Don't show again");

                if ui.button("I Understand").clicked() {
                    self.show_disclaimer = false;
                    if self.disclaimer_checkbox
                        && let Some(storage) = frame.storage_mut()
                    {
                        storage.set_string("show_disclaimer", "false".into());
                    }
                }
            });
        });
    }

    #[cfg(not(target_os = "windows"))]
    fn draw_linux_guide(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        Modal::new(Id::new("ui_linux_guide")).show(ctx, |ui| {
            ui.set_width(240.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Linux Modding").size(18.0));
                ui.vertical_centered(|ui| {
                    ui.label("You must run the Windows game build via Proton. For instructions, follow Step 0 here:");
                    ui.hyperlink_to("SpinShare Modding Guide", GUIDE_URL).on_hover_text(GUIDE_URL);
                    ui.label("and restart this program. (If you are already using Proton, you may disregard this)");
                });

                ui.add_space(8.0);
                ui.checkbox(&mut self.linux_guide_checkbox, "Don't show again");

                if ui.button("Close").clicked() {
                    self.show_linux_guide = false;
                    if self.linux_guide_checkbox
                        && let Some(storage) = frame.storage_mut()
                    {
                        storage.set_string("show_linux_guide", "false".into());
                    }
                }
            });
        });
    }

    fn draw_error_bar(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("ui_error")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("Something went wrong (see output for details)")
                            .color(Color32::RED),
                    );
                    if ui.small_button("Retry").clicked() {
                        self.installer.init();
                    }
                    if ui.small_button("Report an issue").clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab(ISSUES_URL));
                    }
                });
            });
    }

    fn draw_update_bar(&mut self, ctx: &Context) {
        TopBottomPanel::top("ui_update")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Update available for {} mod(s)!",
                            self.updatable_mods.len()
                        ))
                        .color(Color32::from_rgb(90, 170, 255)),
                    );
                    if ui.small_button("Show").clicked() {
                        self.filter_by = FilterBy::Updatable;
                        self.installer.force_ui_update = true;
                    }
                    if ui.small_button("Update All").clicked() {
                        for entry in &self.updatable_mods {
                            self.installer.update_mod(&mut entry.clone().borrow_mut());
                        }
                    }
                });
            });
    }

    fn draw_sidebar(&mut self, ctx: &Context) {
        SidePanel::left("ui_sidebar")
            .exact_width(160.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

                ui.add_space(6.0);
                ui.vertical_centered_justified(|ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(self.installer.state, InstallerState::Ready),
                            |ui| {
                                if ui
                                    .button(RichText::new("Run (Modded)").size(16.0))
                                    .clicked()
                                {
                                    self.installer.patch_game_files();
                                };
                                if ui
                                    .button(RichText::new("Run (Vanilla)").size(16.0))
                                    .clicked()
                                {
                                    self.installer.unpatch_game_files();
                                };
                            },
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new("Categories").size(16.0));
                        Frame::group(ui.style())
                            .fill(ui.visuals().window_fill + Color32::from_gray(6))
                            .show(ui, |ui| {
                                if ui
                                    .toggle_value(&mut self.categories.is_empty(), "All Mods")
                                    .clicked()
                                {
                                    self.categories.clear();
                                    self.installer.force_ui_update = true;
                                }
                                for category in &self.installer.manifest.categories {
                                    if ui
                                        .toggle_value(
                                            &mut self.categories.contains(category),
                                            category,
                                        )
                                        .clicked()
                                    {
                                        if self.categories.contains(category) {
                                            self.categories.remove(category);
                                        } else {
                                            self.categories.insert(category.to_owned());
                                        }
                                        self.installer.force_ui_update = true;
                                    }
                                }
                            });

                        ui.add_space(4.0);
                        ui.label(RichText::new("Output").size(16.0));
                        Frame::group(ui.style())
                            .fill(ui.visuals().window_fill + Color32::from_gray(6))
                            .show(ui, |ui| {
                                ui.set_max_height(ui.available_height() - 26.0);
                                ui.take_available_height();
                                ui.vertical_centered(|ui| {
                                    // ui.set_height((ui.available_height() - 5.0).max(0.0));
                                    ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                                        for msg in &self.installer.log {
                                            ui.label(msg.text());
                                        }
                                    });
                                });
                            });

                        ui.add_space(2.0);
                        ui.add_enabled_ui(
                            matches!(self.installer.state, InstallerState::Ready),
                            |ui| {
                                if ui.button("Open Plugins Folder").clicked() {
                                    let _ = open::that(
                                        self.installer
                                            .dirs
                                            .app_dir
                                            .as_ref()
                                            .unwrap()
                                            .join("BepInEx")
                                            .join("plugins"),
                                    );
                                }
                            },
                        );
                    });
                });
            });
    }

    fn draw_mod_list(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();

            let filter_by_before = self.filter_by;
            let sort_by_before = self.sort_by;

            ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.take_available_space();
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.filter_by, FilterBy::All, "All");
                        ui.selectable_value(&mut self.filter_by, FilterBy::Installed, "Installed");
                        ui.selectable_value(
                            &mut self.filter_by,
                            FilterBy::Uninstalled,
                            "Uninstalled",
                        );

                        ui.label("Sort:");
                        let _ = ComboBox::from_id_salt("ui_sort")
                            .width(80.0)
                            .selected_text(format!("{:?}", self.sort_by))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.sort_by, SortBy::Recent, "Recent");
                                ui.selectable_value(&mut self.sort_by, SortBy::Title, "Title");
                                ui.selectable_value(&mut self.sort_by, SortBy::Author, "Author");
                            });
                        if ui
                            .add(TextEdit::singleline(&mut self.search).hint_text("Search"))
                            .changed()
                        {
                            self.installer.force_ui_update = true;
                        }
                    });

                    if self.filter_by != filter_by_before || self.sort_by != sort_by_before {
                        self.installer.force_ui_update = true;
                    }

                    ui.add_space(4.0);
                    ScrollArea::vertical()
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            ui.take_available_space();
                            let column_width = 400.0;
                            let column_count =
                                ((ui.available_width() / column_width) as usize).max(1);
                            let entries_per_column =
                                self.filtered_mods.len().div_ceil(column_count);

                            ui.columns(column_count, |cols| {
                                for col in 0..cols.len() {
                                    for row in 0..entries_per_column {
                                        let entry = row + col * entries_per_column;
                                        if entry >= self.filtered_mods.len() {
                                            break;
                                        }

                                        self.draw_mod_entry(
                                            &mut cols[col],
                                            &mut self.filtered_mods[entry].clone().borrow_mut(),
                                        );
                                    }
                                }
                            });
                        });
                });
        });
    }

    fn draw_mod_entry(&mut self, ui: &mut egui::Ui, entry: &mut ModEntry) {
        Frame::group(ui.style())
            .fill(ui.visuals().window_fill + Color32::from_gray(6))
            .show(ui, |ui| {
                ui.take_available_width();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.hyperlink_to(
                                RichText::new(entry.entry.name.to_owned()).size(16.0),
                                entry.entry.url.to_owned(),
                            )
                            .on_hover_text(entry.entry.url.to_owned());
                            ui.label(
                                RichText::new(format!("by {}", entry.entry.author.to_owned()))
                                    .weak(),
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(RichText::new(entry.entry.description.to_owned()));

                        if self.show_debug {
                            self.draw_mod_debug(ui, entry);
                        }
                    });

                    let button_width = 100.0;
                    ui.add_space(ui.available_width() - button_width);

                    ui.vertical_centered_justified(|ui| {
                        if entry.state == ModEntryState::PendingInstall
                            || entry.state == ModEntryState::PendingUninstall
                        {
                            ui.disable();
                        }
                        ui.set_width(button_width);
                        let button = ui.button(match entry.state {
                            ModEntryState::Uninstalled => "Install",
                            ModEntryState::PendingInstall => "Downloading",
                            ModEntryState::Installed => "Uninstall",
                            ModEntryState::PendingUninstall => "Removing",
                            ModEntryState::PendingVersionChangeFrom(_) => "Update",
                        });

                        if button.clicked() {
                            match entry.state {
                                ModEntryState::Uninstalled => {
                                    self.installer.install_mod(entry);
                                    self.installer.force_ui_update = true;
                                }
                                ModEntryState::Installed => {
                                    self.installer.uninstall_mod(entry);
                                    self.installer.force_ui_update = true;
                                }
                                ModEntryState::PendingVersionChangeFrom(_) => {
                                    self.installer.update_mod(entry);
                                    self.installer.force_ui_update = true;
                                }
                                _ => {}
                            }
                        }

                        ui.vertical_centered(|ui| {
                            ComboBox::from_id_salt(entry.entry.name.to_owned())
                                .selected_text(match entry.state {
                                    ModEntryState::PendingVersionChangeFrom(current_version) => {
                                        format!(
                                            "{} ({})",
                                            format_version(
                                                &entry.entry.versions[current_version].name,
                                            ),
                                            format_version(
                                                &entry.entry.versions[entry.selected_version].name,
                                            )
                                        )
                                    }
                                    _ => format_version(
                                        &entry.entry.versions[entry.selected_version].name,
                                    ),
                                })
                                .show_ui(ui, |ui| {
                                    for i in 0..entry.entry.versions.len() {
                                        let option = ui.selectable_label(
                                            i == entry.selected_version,
                                            entry.entry.versions[i].name.to_owned(),
                                        );

                                        if option.clicked() {
                                            entry.set_version(i);
                                        }
                                    }
                                });
                        });
                    });
                });
            });
    }

    fn draw_mod_debug(&self, ui: &mut egui::Ui, entry: &mut ModEntry) {
        ui.label(
            RichText::new(format!("Categories: {}", entry.entry.categories.join(", "))).weak(),
        );
        ui.label(
            RichText::new(format!(
                "Dependencies: {}",
                entry.entry.dependencies.join(", ")
            ))
            .weak(),
        );
        ui.label(RichText::new(format!("Dependent Count: {}", entry.active_dependents)).weak());
        ui.label(RichText::new(entry.entry.versions[entry.selected_version].digest.clone()).weak());
    }

    fn build_list(&mut self) {
        self.updatable_mods = self
            .installer
            .mods
            .iter()
            .filter(|entry| {
                matches!(
                    entry.borrow().state,
                    ModEntryState::PendingVersionChangeFrom(_)
                )
            })
            .cloned()
            .collect();

        self.filtered_mods = self
            .installer
            .mods
            .iter()
            .filter(|entry| {
                entry
                    .borrow()
                    .entry
                    .name
                    .to_ascii_lowercase()
                    .contains(self.search.to_ascii_lowercase().trim())
            })
            .filter(|entry| match self.filter_by {
                FilterBy::All => true,
                FilterBy::Installed => matches!(
                    entry.borrow().state,
                    ModEntryState::Installed
                        | ModEntryState::PendingUninstall
                        | ModEntryState::PendingVersionChangeFrom(_)
                ),
                FilterBy::Uninstalled => matches!(
                    entry.borrow().state,
                    ModEntryState::Uninstalled | ModEntryState::PendingInstall
                ),
                FilterBy::Updatable => matches!(
                    entry.borrow().state,
                    ModEntryState::PendingVersionChangeFrom(_)
                ),
            })
            .filter(|entry| {
                self.categories
                    .iter()
                    .all(|category| entry.borrow().entry.categories.contains(category))
            })
            .filter(|entry| !entry.borrow().entry.versions.is_empty())
            .cloned()
            .collect();

        self.filtered_mods.sort_by(|a, b| {
            let a = a.borrow();
            let b = b.borrow();
            match self.sort_by {
                SortBy::Recent => b.entry.versions[0]
                    .created_at
                    .cmp(&a.entry.versions[0].created_at),
                SortBy::Title => a.entry.name.cmp(&b.entry.name),
                SortBy::Author => a.entry.author.cmp(&b.entry.author),
            }
        });
    }
}

fn format_version(version: &str) -> String {
    format!(
        "v{}",
        version
            .chars()
            .filter(|c| c.is_numeric() || matches!(c, '.' | ','))
            .collect::<String>()
    )
}

fn load_font() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "ShareTech".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "./assets/ShareTech-Regular.ttf"
        ))),
    );

    fonts
        .families
        .get_mut(&egui::epaint::FontFamily::Proportional)
        .expect("Failed to access Fonts")
        .insert(0, "ShareTech".to_owned());

    fonts
}
