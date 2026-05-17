use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, Stroke, Vec2, RichText, Sense, ScrollArea};
use egui_shadcn::{button, ControlSize, ControlVariant, Theme};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 820.0])
            .with_title("Kanban"),
        ..Default::default()
    };
    eframe::run_native("Kanban", options, Box::new(|_| Ok(Box::new(KanbanApp::default()))))
}

#[derive(Default)]
struct KanbanApp {
    theme: Theme,
}


struct Column {
    title: &'static str,
    color: Color32,
    desc: &'static str,
    show_add_item: bool,
}

impl eframe::App for KanbanApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(8, 8, 12);
        ctx.set_visuals(visuals);

        let columns = [
            Column { title: "Todo",        color: Color32::from_rgb(34, 197, 94),  desc: "This item hasn't been started",    show_add_item: false },
            Column { title: "In Progress", color: Color32::from_rgb(234, 179, 8),  desc: "This is actively being worked on", show_add_item: false },
            Column { title: "Done",        color: Color32::from_rgb(139, 92, 246), desc: "This has been completed",          show_add_item: true  },
        ];

        let theme = self.theme.clone();

        egui::CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(8, 8, 12)))
            .show(ctx, |ui| {
                ui.add_space(24.0);
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_space(24.0);

                        for col in &columns {
                            Frame::new()
                                .fill(Color32::from_rgb(13, 13, 18))
                                .corner_radius(CornerRadius::same(10))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(28, 28, 38)))
                                .inner_margin(Margin::same(18))
                                .show(ui, |ui| {
                                    ui.set_width(420.0);
                                    ui.set_min_height(740.0);

                                    // Header
                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                        ui.painter().circle_stroke(rect.center(), 9.0, Stroke::new(2.0, col.color));

                                        ui.add_space(4.0);
                                        ui.label(RichText::new(col.title).color(Color32::WHITE).size(15.0).strong());
                                        ui.add_space(6.0);

                                        button(ui, &theme, "0", ControlVariant::Outline, ControlSize::Sm, false);

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            button(ui, &theme, "+",   ControlVariant::Ghost, ControlSize::Sm, true);
                                            button(ui, &theme, "···", ControlVariant::Ghost, ControlSize::Sm, true);
                                        });
                                    });

                                    ui.add_space(10.0);
                                    ui.label(RichText::new(col.desc).color(Color32::from_rgb(80, 80, 100)).size(13.0));

                                    if col.show_add_item {
                                        ui.add_space(18.0);
                                        button(ui, &theme, "+ Add item", ControlVariant::Ghost, ControlSize::Sm, true);
                                    }
                                });

                            ui.add_space(18.0);
                        }

                        ui.add_space(24.0);
                    });
                });
            });
    }
}