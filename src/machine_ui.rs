//! Machine tab (connection, position, jogging, zeroing, job) and the console tab.

use crate::app::App;
use crate::machine::{self, Cmd, LogKind};
use crate::panels::{len_edit, section, unit_k};
use egui::{Color32, RichText, Ui};

impl App {
    pub fn machine_tab(&mut self, ui: &mut Ui, jog: bool) {
        if !jog {
        section(ui, "Connection");
        ui.horizontal(|ui| {
            let port = self.settings.machine.port.clone();
            egui::ComboBox::from_id_salt("port").width(150.0).selected_text(port).show_ui(ui, |ui| {
                for p in &self.ports {
                    ui.selectable_value(&mut self.settings.machine.port, p.clone(), p);
                }
            });
            if ui.button("⟳").on_hover_text("Look for machines again").clicked() {
                self.ports = machine::list_ports();
            }
            if self.connected.is_some() {
                if ui.button("Disconnect").clicked() {
                    self.machine.send(Cmd::Disconnect);
                }
            } else if ui.add(egui::Button::new(RichText::new("Connect").strong()).fill(Color32::from_rgb(40, 120, 70))).clicked() {
                self.machine.send(Cmd::Connect(self.settings.machine.port.clone(), self.settings.machine.baud));
            }
        });
        ui.label(RichText::new(format!("{} · {} baud · \"Simulator\" lets you try everything without a machine.", self.settings.machine.name, self.settings.machine.baud)).size(11.0).color(Color32::from_gray(150)));
        if self.settings.machine.port == machine::SIMULATOR {
            ui.horizontal(|ui| {
                ui.label("Simulator speed");
                for s in [1.0, 2.0, 5.0, 10.0, 50.0] {
                    if ui.selectable_label((self.design.sim_speed - s).abs() < 1e-9, format!("{s}×")).on_hover_text("How much faster than a real machine the simulator runs").clicked() {
                        self.design.sim_speed = s;
                        self.machine.send(Cmd::SimSpeed(s));
                    }
                }
            });
        }

        }
        // position readout
        section(ui, "Position");
        let state_col = match self.mstate.split(':').next().unwrap_or("") {
            "Idle" => Color32::from_rgb(90, 210, 110),
            "Run" | "Jog" | "Home" => Color32::from_rgb(70, 160, 255),
            "Hold" | "Door" => Color32::from_rgb(250, 200, 70),
            "Alarm" => Color32::from_rgb(240, 80, 80),
            _ => Color32::from_gray(150),
        };
        let k = unit_k(self.settings.units);
        egui::Frame::default().fill(Color32::from_rgb(22, 25, 29)).corner_radius(4.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("⏺ {}", self.mstate)).color(state_col).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("F {:.0}  S {:.0}", self.mfeed, self.mspindle)).monospace().weak());
                });
            });
            egui::Grid::new("dro").num_columns(3).spacing([16.0, 2.0]).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new("Work").weak());
                ui.label(RichText::new("Machine").weak());
                ui.end_row();
                for (i, a) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(RichText::new(*a).strong().size(16.0));
                    ui.label(RichText::new(format!("{:9.3}", self.wpos[i] / k)).monospace().size(17.0).color(Color32::from_rgb(120, 230, 160)));
                    ui.label(RichText::new(format!("{:9.3}", self.mpos[i] / k)).monospace().weak());
                    ui.end_row();
                }
            });
        });

        if jog {
        section(ui, "Move");
        ui.horizontal(|ui| {
            ui.label("Step");
            for s in [0.1, 1.0, 5.0, 10.0, 50.0] {
                if ui.selectable_label((self.jog_step - s).abs() < 1e-9, format!("{s}")).clicked() {
                    self.jog_step = s;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Z step");
            for s in [0.05, 0.1, 0.5, 1.0, 5.0] {
                if ui.selectable_label((self.jog_z_step - s).abs() < 1e-9, format!("{s}")).clicked() {
                    self.jog_z_step = s;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Speed");
            ui.add(egui::DragValue::new(&mut self.jog_feed).speed(10.0).range(10.0..=20000.0).suffix(" mm/min"));
        });
        let st = self.jog_step;
        let zs = self.jog_z_step;
        let mut jog: Option<(f64, f64, f64)> = None;
        ui.horizontal(|ui| {
            let b = egui::vec2(40.0, 34.0);
            egui::Grid::new("jog").spacing([4.0, 4.0]).show(ui, |ui| {
                if ui.add_sized(b, egui::Button::new("↖")).clicked() {
                    jog = Some((-st, st, 0.0));
                }
                if ui.add_sized(b, egui::Button::new("Y+")).clicked() {
                    jog = Some((0.0, st, 0.0));
                }
                if ui.add_sized(b, egui::Button::new("↗")).clicked() {
                    jog = Some((st, st, 0.0));
                }
                ui.end_row();
                if ui.add_sized(b, egui::Button::new("X-")).clicked() {
                    jog = Some((-st, 0.0, 0.0));
                }
                if ui.add_sized(b, egui::Button::new("XY0")).on_hover_text("Go to work zero (X/Y)").clicked() {
                    self.send_line("G90 G0 X0 Y0");
                }
                if ui.add_sized(b, egui::Button::new("X+")).clicked() {
                    jog = Some((st, 0.0, 0.0));
                }
                ui.end_row();
                if ui.add_sized(b, egui::Button::new("↙")).clicked() {
                    jog = Some((-st, -st, 0.0));
                }
                if ui.add_sized(b, egui::Button::new("Y-")).clicked() {
                    jog = Some((0.0, -st, 0.0));
                }
                if ui.add_sized(b, egui::Button::new("↘")).clicked() {
                    jog = Some((st, -st, 0.0));
                }
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                if ui.add_sized(b, egui::Button::new("Z+")).clicked() {
                    jog = Some((0.0, 0.0, zs));
                }
                ui.add_space(b.y + 4.0);
                if ui.add_sized(b, egui::Button::new("Z-")).clicked() {
                    jog = Some((0.0, 0.0, -zs));
                }
            });
        });
        if let Some((x, y, z)) = jog {
            self.jog(x, y, z);
        }

        section(ui, "Zero and home");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Set X/Y zero").on_hover_text("Make the current spot the job's X/Y origin").clicked() {
                self.send_line("G10 L20 P1 X0 Y0");
            }
            if ui.button("Set Z zero").clicked() {
                self.send_line("G10 L20 P1 Z0");
            }
            if ui.button("Zero all").clicked() {
                self.send_line("G10 L20 P1 X0 Y0 Z0");
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("🏠 Home").clicked() {
                self.send_line("$H");
            }
            if ui.button("🔓 Unlock").clicked() {
                self.send_line("$X");
            }
            if ui.button("Reset").on_hover_text("Soft reset of the controller").clicked() {
                self.realtime(0x18);
            }
            if ui.button("Raise Z").on_hover_text("Move up to the safe height").clicked() {
                let z = self.settings.machine.safe_z + self.doc.stock.top_z();
                self.send_line(&format!("G90 G0 Z{z:.3}"));
            }
        });
        ui.horizontal(|ui| {
            ui.label("Touch plate");
            len_edit(ui, &mut self.settings.plate_thickness, self.settings.units, 0.05);
            if ui.button("Probe Z").on_hover_text("Lower the bit onto the touch plate and set Z zero").clicked() {
                let t = self.settings.plate_thickness;
                for l in ["G91 G38.2 Z-50 F100".to_string(), format!("G10 L20 P1 Z{t:.3}"), "G0 Z5".to_string(), "G90".to_string()] {
                    self.send_line(&l);
                }
            }
        });

        }
        if !jog {
        section(ui, "Spindle");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.spindle_rpm).speed(100.0).range(0.0..=60000.0).suffix(" rpm"));
            if ui.button("On").clicked() {
                let s = format!("M3 S{:.0}", self.spindle_rpm);
                self.send_line(&s);
            }
            if ui.button("Off").clicked() {
                self.send_line("M5");
            }
        });

        section(ui, "Job");
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.job_running, egui::Button::new(RichText::new("▶ Start").strong()).fill(Color32::from_rgb(40, 140, 70))).clicked() {
                self.start_job();
            }
            if ui.button(if self.paused { "▶ Resume" } else { "⏸ Pause" }).clicked() {
                self.realtime(if self.paused { b'~' } else { b'!' });
                self.paused = !self.paused;
            }
            if ui.add(egui::Button::new("⏹ Stop").fill(Color32::from_rgb(150, 45, 45))).clicked() {
                self.stop_job();
            }
            if ui.button("⛶ Frame").clicked() {
                self.frame_job();
            }
        });
        if let Some((a, b)) = self.job_progress {
            let f = if b > 0 { a as f32 / b as f32 } else { 0.0 };
            ui.add(egui::ProgressBar::new(f).text(format!("{a} / {b} lines")));
        }
        ui.horizontal(|ui| {
            if ui.button("Resume from…").on_hover_text("Start the job part-way through, after a broken bit or a power cut").clicked() {
                self.design.resume_open = true;
            }
            if ui.button("Machine settings…").clicked() {
                self.settings_open = true;
            }
        });
        }
    }

    pub fn console_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (l, c) in [("$$", "Settings"), ("$#", "Offsets"), ("$G", "Modes"), ("$I", "Info")] {
                if ui.button(l).on_hover_text(c).clicked() {
                    self.send_line(l);
                }
            }
            if ui.button("Clear").clicked() {
                self.console.clear();
            }
        });
        let mut send = false;
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.console_input).desired_width(ui.available_width() - 60.0).hint_text("Type a command").font(egui::TextStyle::Monospace));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                send = true;
                r.request_focus();
            }
            if ui.button("Send").clicked() {
                send = true;
            }
        });
        if send && !self.console_input.trim().is_empty() {
            let l = self.console_input.trim().to_string();
            self.send_line(&l);
            if self.connected.is_none() {
                self.log("Not connected - open the Machine tab and press Connect.", LogKind::Error);
            }
            self.console_input.clear();
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("console").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (l, k) in &self.console {
                let (prefix, col) = match k {
                    LogKind::Sent => ("› ", Color32::from_rgb(120, 180, 255)),
                    LogKind::Received => ("  ", Color32::from_gray(210)),
                    LogKind::Info => ("• ", Color32::from_rgb(140, 200, 140)),
                    LogKind::Error => ("! ", Color32::from_rgb(250, 110, 100)),
                };
                ui.label(RichText::new(format!("{prefix}{l}")).monospace().color(col));
            }
        });
    }
}
