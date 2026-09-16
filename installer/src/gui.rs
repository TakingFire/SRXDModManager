use eframe::egui::{
    self, CentralPanel, Color32, ComboBox, Frame, Id, Modal, OpenUrl, Panel, RichText, ScrollArea,
    Stroke, TextEdit, Ui,
};

use crate::app::{Installer, InstallerState, ModEntry, ModEntryRef, ModEntryState};
use crate::config::{FilterBy, PopupState, SortBy};

const GUIDE_URL: &str = "https://useredge.github.io/spinshare-wiki/modding/installation-guide/";
const UPDATE_URL: &str = "https://github.com/TakingFire/SRXDModManager/releases/latest";
const ISSUES_URL: &str = "https://github.com/TakingFire/SRXDModManager/issues/new";

#[derive(Default)]
pub struct Gui {
    pub installer: Installer,

    filtered_mods: Vec<ModEntryRef>,
    updatable_mods: Vec<ModEntryRef>,
    unrecognized_mods: Vec<ModEntryRef>,

    disclaimer_checkbox: bool,
    linux_guide_checkbox: bool,
    existing_config_checkbox: bool,

    show_settings: bool,

    initialized: bool,
    show_debug: bool,
}

impl eframe::App for Gui {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "config", &self.installer.config);
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.input(|input| {
            self.show_debug = input.key_down(egui::Key::AltLeft);
        });

        if !self.initialized {
            if let Some(storage) = frame.storage_mut() {
                self.load(storage);
            }

            if self.installer.config.show_app_console {
                crate::show_console(true);
            }

            ctx.set_fonts(load_font());
            self.build_list();

            self.installer.config.popup_linux_guide.enable();
            self.installer.config.popup_disclaimer.enable();

            self.initialized = true;
        }

        self.installer.update();

        if self.installer.force_ui_update {
            self.build_list();
            ctx.request_repaint();
            self.installer.force_ui_update = false;
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let config = &mut self.installer.config;

        match () {
            () if matches!(self.installer.state, InstallerState::Launching) => {
                self.draw_launching_popup(ui);
            }

            () if matches!(self.installer.state, InstallerState::Outdated) => {
                self.draw_outdated_warning(ui);
            }

            () if cfg!(not(target_os = "windows"))
                && matches!(config.popup_linux_guide, PopupState::Active) =>
            {
                self.draw_linux_guide(ui);
            }

            () if matches!(config.popup_disclaimer, PopupState::Active) => {
                self.draw_disclaimer(ui);
            }

            () if matches!(config.popup_existing_config, PopupState::Active) => {
                self.draw_config_popup(ui);
            }

            () => {}
        }

        if self.show_settings {
            self.draw_settings_menu(ui);
        }

        if matches!(self.installer.state, InstallerState::Error) {
            self.draw_error_bar(ui);
        }

        self.draw_sidebar(ui);

        if self.installer.config.show_outdated_mods && !self.updatable_mods.is_empty() {
            self.draw_update_bar(ui);
        }

        if self.installer.config.show_unrecognized_mods && !self.unrecognized_mods.is_empty() {
            self.draw_unrecognized_bar(ui);
        }

        self.draw_mod_list(ui);

        ui.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl Gui {
    fn load(&mut self, storage: &dyn eframe::Storage) {
        if let Some(config) = eframe::get_value(storage, "config") {
            self.installer.config = config;
        }
    }

    fn draw_outdated_warning(&mut self, ui: &Ui) {
        Modal::new(Id::new("ui_disclaimer")).show(ui, |ui| {
            ui.set_width(240.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("popup.outdated.title")).size(18.0));
                ui.label(t!("popup.outdated.text"));
                ui.hyperlink_to(t!("popup.outdated.link"), UPDATE_URL);

                ui.add_space(8.0);
                ui.vertical_centered_justified(|ui| {
                    if ui.button(t!("popup.outdated.button")).clicked() {
                        self.installer.state = InstallerState::Init;
                        self.installer.get_patcher();
                    }
                });
            });
        });
    }

    fn draw_disclaimer(&mut self, ui: &Ui) {
        Modal::new(Id::new("ui_disclaimer")).show(ui, |ui| {
            ui.set_width(220.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("popup.disclaimer.title")).size(18.0));
                ui.label(t!("popup.disclaimer.text"));

                ui.add_space(8.0);
                ui.checkbox(&mut self.disclaimer_checkbox, t!("button.disable_popup"));

                ui.vertical_centered_justified(|ui| {
                    if ui.button(t!("popup.disclaimer.button")).clicked() {
                        if self.disclaimer_checkbox {
                            self.installer.config.popup_disclaimer.disable();
                        } else {
                            self.installer.config.popup_disclaimer.dismiss();
                        }
                    }
                });
            });
        });
    }

    fn draw_linux_guide(&mut self, ui: &Ui) {
        Modal::new(Id::new("ui_linux_guide")).show(ui, |ui| {
            ui.set_width(240.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("popup.linux.title")).size(18.0));
                ui.vertical_centered(|ui| {
                    ui.label(t!("popup.linux.text1"));
                    ui.hyperlink_to(t!("popup.linux.link"), GUIDE_URL)
                        .on_hover_text(GUIDE_URL);
                    ui.label(t!("popup.linux.text2"));
                });

                ui.add_space(8.0);
                ui.checkbox(&mut self.linux_guide_checkbox, t!("button.disable_popup"));

                ui.vertical_centered_justified(|ui| {
                    if ui.button(t!("popup.linux.button")).clicked() {
                        if self.linux_guide_checkbox {
                            self.installer.config.popup_linux_guide.disable();
                        } else {
                            self.installer.config.popup_linux_guide.dismiss();
                        }
                    }
                });
            });
        });
    }

    fn draw_config_popup(&mut self, ui: &Ui) {
        Modal::new(Id::new("ui_config")).show(ui, |ui| {
            ui.set_width(230.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("popup.existing_config.title")).size(18.0));
                ui.label(t!("popup.existing_config.text1"));
                ui.label(t!("popup.existing_config.text2"));

                ui.add_space(8.0);
                ui.checkbox(
                    &mut self.existing_config_checkbox,
                    t!("button.disable_popup"),
                );

                ui.columns(2, |cols| {
                    cols[0].vertical_centered_justified(|ui| {
                        if ui.button(t!("popup.existing_config.btn_cancel")).clicked() {
                            if self.existing_config_checkbox {
                                self.installer.config.popup_existing_config.disable();
                            } else {
                                self.installer.config.popup_existing_config.dismiss();
                            }
                        }
                    });

                    cols[1].vertical_centered_justified(|ui| {
                        if ui.button(t!("popup.existing_config.btn_copy")).clicked() {
                            self.installer.config.popup_existing_config.disable();
                            self.installer.copy_existing_config();
                        }
                    });
                });
            });
        });
    }

    fn draw_launching_popup(&self, ui: &Ui) {
        Modal::new(Id::new("ui_disclaimer")).show(ui, |ui| {
            ui.set_width(240.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("popup.launching.title")).size(18.0));
                ui.add_space(8.0);
                ui.add(egui::Spinner::new().color(ui.visuals().text_color()));
                ui.add_space(8.0);
                ui.label(t!("popup.launching.text"));
            });
        });
    }

    fn draw_error_bar(&mut self, ui: &mut Ui) {
        Panel::bottom("ui_error").exact_size(28.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(t!("popup.error.text")).color(Color32::RED));
                if ui.small_button(t!("popup.error.btn_retry")).clicked() {
                    self.installer.init();
                }
                if ui.small_button(t!("popup.error.btn_report")).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(ISSUES_URL));
                }
            });
        });
    }

    fn draw_update_bar(&mut self, ui: &mut Ui) {
        Panel::top("ui_update").exact_size(28.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    RichText::new(t!(
                        "popup.mod_update.text",
                        count = self.updatable_mods.len()
                    ))
                    .color(Color32::from_rgb(90, 170, 255)),
                );
                if ui.small_button(t!("popup.mod_update.btn_show")).clicked() {
                    self.installer.config.filter_by = FilterBy::Updatable;
                    self.installer.force_ui_update = true;
                }
                if ui.small_button(t!("popup.mod_update.btn_update")).clicked() {
                    for entry in &self.updatable_mods {
                        self.installer.update_mod(&mut entry.clone().borrow_mut());
                    }
                }
            });
        });
    }

    fn draw_unrecognized_bar(&mut self, ui: &mut Ui) {
        Panel::top("ui_unrecognized")
            .exact_size(28.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(t!(
                            "popup.mod_unrecognized.text",
                            count = self.unrecognized_mods.len()
                        ))
                        .color(Color32::from_rgb(255, 160, 80)),
                    );
                    if ui
                        .small_button(t!("popup.mod_unrecognized.btn_show"))
                        .clicked()
                    {
                        self.installer.config.filter_by = FilterBy::Unrecognized;
                        self.installer.force_ui_update = true;
                    }
                    if ui
                        .small_button(t!("popup.mod_unrecognized.btn_remove"))
                        .clicked()
                    {
                        for entry in &self.unrecognized_mods {
                            self.installer
                                .uninstall_mod(&mut entry.clone().borrow_mut(), true);
                        }
                    }
                });
            });
    }

    fn draw_sidebar(&mut self, ui: &mut Ui) {
        Panel::left("ui_sidebar")
            .exact_size(160.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

                ui.add_space(6.0);
                ui.vertical_centered_justified(|ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(self.installer.state, InstallerState::Ready),
                            |ui| {
                                if ui
                                    .button(RichText::new(t!("button.run_modded")).size(16.0))
                                    .clicked()
                                {
                                    self.installer.patch_game_files();
                                }
                                if ui
                                    .button(RichText::new(t!("button.run_vanilla")).size(16.0))
                                    .clicked()
                                {
                                    self.installer.unpatch_game_files();
                                }
                            },
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new(t!("label.categories")).size(16.0));
                        Frame::group(ui.style())
                            .fill(ui.visuals().window_fill + Color32::from_gray(6))
                            .show(ui, |ui| {
                                let config = &mut self.installer.config;

                                if ui
                                    .toggle_value(
                                        &mut config.categories.is_empty(),
                                        t!("label.category_all"),
                                    )
                                    .clicked()
                                {
                                    config.categories.clear();
                                    self.installer.force_ui_update = true;
                                }
                                for category in &config.manifest.categories {
                                    if ui
                                        .toggle_value(
                                            &mut config.categories.contains(category),
                                            category.clone(),
                                        )
                                        .clicked()
                                    {
                                        if config.categories.contains(category) {
                                            config.categories.remove(category);
                                        } else {
                                            config.categories.insert(category.to_owned());
                                        }
                                        self.installer.force_ui_update = true;
                                    }
                                }
                            });

                        ui.add_space(4.0);
                        ui.label(RichText::new(t!("label.output")).size(16.0));
                        Frame::group(ui.style())
                            .fill(ui.visuals().window_fill + Color32::from_gray(6))
                            .show(ui, |ui| {
                                ui.set_max_height(ui.available_height() - 26.0);
                                ui.take_available_space();
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
                        if ui.button(t!("button.open_settings")).clicked() {
                            self.show_settings = true;
                        }
                    });
                });
            });
    }

    fn draw_mod_list(&mut self, ui: &mut Ui) {
        let style = Frame::central_panel(ui.style());
        let padding = egui::Margin {
            bottom: 0,
            ..style.inner_margin
        };

        CentralPanel::default()
            .frame(style.inner_margin(padding))
            .show(ui, |ui| {
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();

                ScrollArea::horizontal()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        self.draw_filter_bar(ui);

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
                                    for (col, ui) in cols.iter_mut().enumerate() {
                                        for row in 0..entries_per_column {
                                            let entry = row * column_count + col;
                                            if entry >= self.filtered_mods.len() {
                                                break;
                                            }

                                            self.draw_mod_entry(
                                                ui,
                                                &mut self.filtered_mods[entry].clone().borrow_mut(),
                                            );
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                            });
                    });
            });
    }

    fn draw_filter_bar(&mut self, ui: &mut Ui) {
        let config = &mut self.installer.config;

        let filter_by_before = config.filter_by;
        let sort_by_before = config.sort_by;

        ui.take_available_space();
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut config.filter_by,
                FilterBy::All,
                t!("config.filter.all"),
            );
            ui.selectable_value(
                &mut config.filter_by,
                FilterBy::Installed,
                t!("config.filter.installed"),
            );
            ui.selectable_value(
                &mut config.filter_by,
                FilterBy::Uninstalled,
                t!("config.filter.uninstalled"),
            );

            ui.label(t!("label.sort"));
            let _ = ComboBox::from_id_salt("ui_sort")
                .width(80.0)
                .selected_text(match config.sort_by {
                    SortBy::Recent => t!("config.sort.recent"),
                    SortBy::Title => t!("config.sort.title"),
                    SortBy::Author => t!("config.sort.author"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut config.sort_by,
                        SortBy::Recent,
                        t!("config.sort.recent"),
                    );
                    ui.selectable_value(
                        &mut config.sort_by,
                        SortBy::Title,
                        t!("config.sort.title"),
                    );
                    ui.selectable_value(
                        &mut config.sort_by,
                        SortBy::Author,
                        t!("config.sort.author"),
                    );
                });
            if ui
                .add(TextEdit::singleline(&mut config.search).hint_text(t!("label.search")))
                .changed()
            {
                self.installer.force_ui_update = true;
            }
        });

        if config.filter_by != filter_by_before || config.sort_by != sort_by_before {
            self.installer.force_ui_update = true;
        }
    }

    fn draw_mod_entry(&mut self, ui: &mut Ui, entry: &mut ModEntry) {
        let accent_color = if !entry.recognized {
            Color32::from_rgb(255, 160, 80)
        } else if matches!(
            entry.state,
            ModEntryState::Installed | ModEntryState::PendingVersionChangeFrom(_)
        ) {
            Color32::from_rgb(90, 170, 255)
        } else {
            Color32::TRANSPARENT
        };

        let fill_color = ui.visuals().window_fill + Color32::from_gray(6);
        let border_color = ui.visuals().window_stroke.color;

        Frame::group(ui.style())
            .fill(fill_color.blend(accent_color.gamma_multiply(0.0625)))
            .stroke(Stroke {
                color: border_color.blend(accent_color.gamma_multiply(0.25)),
                ..ui.visuals().window_stroke
            })
            .show(ui, |ui| {
                ui.take_available_width();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.hyperlink_to(
                                RichText::new(entry.entry.name.clone()).size(16.0),
                                entry.entry.url.clone(),
                            )
                            .on_hover_text(entry.entry.url.clone());
                            ui.label(
                                RichText::new(t!(
                                    "modentry.author",
                                    name = entry.entry.author.clone()
                                ))
                                .weak(),
                            );
                        });

                        if entry.recognized {
                            ui.add_space(2.0);
                            ui.label(RichText::new(entry.entry.description.clone()));
                        }

                        if self.show_debug {
                            self.draw_mod_debug(ui, entry);
                        }
                    });

                    let button_width = 100.0;
                    ui.add_space(ui.available_width() - button_width);

                    ui.vertical_centered_justified(|ui| {
                        if matches!(entry.state, ModEntryState::PendingInstall)
                            || matches!(entry.state, ModEntryState::PendingUninstall)
                        {
                            ui.disable();
                        }
                        ui.set_width(button_width);
                        let button = ui.button(match entry.state {
                            ModEntryState::Uninstalled => t!("modentry.button.install"),
                            ModEntryState::PendingInstall => t!("modentry.button.downloading"),
                            ModEntryState::Installed => t!("modentry.button.uninstall"),
                            ModEntryState::PendingUninstall => t!("modentry.button.removing"),
                            ModEntryState::PendingVersionChangeFrom(_) => {
                                t!("modentry.button.update")
                            }
                        });

                        if button.clicked() {
                            match entry.state {
                                ModEntryState::Uninstalled => {
                                    self.installer.install_mod(entry);
                                    self.installer.force_ui_update = true;
                                }
                                ModEntryState::Installed => {
                                    self.installer.uninstall_mod(entry, false);
                                    self.installer.force_ui_update = true;
                                }
                                ModEntryState::PendingVersionChangeFrom(_) => {
                                    self.installer.update_mod(entry);
                                    self.installer.force_ui_update = true;
                                }
                                _ => {}
                            }
                        }

                        if !entry.recognized {
                            return;
                        }

                        ComboBox::from_id_salt(entry.entry.name.clone())
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
                                            entry.entry.versions[i].name.clone(),
                                        );

                                        if option.clicked() {
                                            entry.set_version(i);
                                        }
                                    }
                                });

                        if self.show_debug
                            && matches!(
                                entry.state,
                                ModEntryState::Installed
                                    | ModEntryState::PendingVersionChangeFrom(_)
                            )
                            && ui.button(t!("modentry.button.open_folder")).clicked()
                        {
                            let _ = open::that(
                                self.installer
                                    .dirs
                                    .app_dir
                                    .as_ref()
                                    .unwrap()
                                    .join("BepInEx")
                                    .join("plugins")
                                    .join(&entry.entry.versions[entry.selected_version].digest),
                            );
                        }
                    });
                });
            });
    }

    fn draw_mod_debug(&self, ui: &mut Ui, entry: &ModEntry) {
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

    fn draw_settings_menu(&mut self, ui: &Ui) {
        Modal::new(Id::new("ui_disclaimer")).show(ui, |ui| {
            ui.set_max_width(300.0);
            // ui.set_width(300.0_f32.min(ui.content_rect().width() - 32.0));

            ui.vertical_centered_justified(|ui| {
                ui.label(RichText::new(t!("settings.title")).size(18.0));
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    cols[0].vertical_centered_justified(|ui| {
                        let dirs = &self.installer.dirs;

                        ui.add_enabled_ui(
                            matches!(self.installer.state, InstallerState::Ready),
                            |ui| {
                                if ui.button(t!("settings.btn_plugins_folder")).clicked() {
                                    let _ = open::that(
                                        dirs.app_dir.as_ref().unwrap().join("BepInEx/plugins"),
                                    );
                                }
                                if ui.button(t!("settings.btn_game_folder")).clicked() {
                                    let _ = open::that(dirs.game_dir.as_ref().unwrap());
                                }
                                if ui.button(t!("settings.btn_steam_folder")).clicked() {
                                    let _ = open::that(dirs.steam_dir.as_ref().unwrap());
                                }
                            },
                        );

                        ui.add_space(8.0);

                        ui.style_mut().visuals.widgets.inactive.weak_bg_fill =
                            Color32::TRANSPARENT.blend(Color32::RED.gamma_multiply(0.125));
                        ui.style_mut().visuals.widgets.hovered.weak_bg_fill =
                            Color32::TRANSPARENT.blend(Color32::RED.gamma_multiply(0.25));

                        if ui.button(t!("settings.btn_remove_mods")).clicked() {
                            self.installer.uninstall_all_mods();
                        }
                        if ui.button(t!("settings.btn_reset_app")).clicked() {
                            self.reset_app(ui);
                        }
                    });

                    cols[1].vertical(|ui| {
                        let config = &mut self.installer.config;

                        ui.checkbox(
                            &mut config.show_game_console,
                            t!("settings.show_game_console"),
                        );
                        ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
                            if ui
                                .checkbox(
                                    &mut config.show_app_console,
                                    t!("settings.show_app_console"),
                                )
                                .changed()
                            {
                                crate::show_console(config.show_app_console);
                            }
                        });

                        ui.add_space(8.0);

                        ui.checkbox(
                            &mut config.show_unrecognized_mods,
                            t!("settings.show_unrecognized_mods"),
                        );
                        ui.checkbox(
                            &mut config.show_outdated_mods,
                            t!("settings.show_outdated_mods"),
                        );
                        ui.checkbox(
                            &mut config.show_outdated_app,
                            t!("settings.show_outdated_app"),
                        );
                    });
                });

                ui.add_space(8.0);
                if ui.button(t!("settings.btn_close")).clicked() {
                    self.show_settings = false;
                }
            });
        });
    }

    fn build_list(&mut self) {
        let config = &mut self.installer.config;

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

        self.unrecognized_mods = self
            .installer
            .mods
            .iter()
            .filter(|entry| !entry.borrow().recognized)
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
                    .contains(config.search.to_ascii_lowercase().trim())
            })
            .filter(|entry| match config.filter_by {
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
                FilterBy::Unrecognized => !entry.borrow().recognized,
            })
            .filter(|entry| {
                config
                    .categories
                    .iter()
                    .all(|category| entry.borrow().entry.categories.contains(category))
            })
            .filter(|entry| !entry.borrow().entry.versions.is_empty())
            .cloned()
            .collect();

        self.filtered_mods.sort_by(|a, b| {
            let a = a.borrow();
            let b = b.borrow();
            match config.sort_by {
                SortBy::Recent => b.entry.versions[0]
                    .created_at
                    .cmp(&a.entry.versions[0].created_at),
                SortBy::Title => a.entry.name.cmp(&b.entry.name),
                SortBy::Author => a.entry.author.cmp(&b.entry.author),
            }
        });
    }

    fn reset_app(&mut self, ui: &Ui) {
        ui.memory_mut(|memory| {
            *memory = egui::Memory::default();
        });

        let mut installer = Installer::default();
        installer.init();

        *self = Self::default();
        self.installer = installer;
        self.initialized = true;
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
