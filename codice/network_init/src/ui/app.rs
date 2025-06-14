use common::client_ui::ClientUI;
use eframe::egui;
use egui::{ViewportBuilder, ViewportId};
use simulation_controller::SimulationControllerUI;

/// Main application state
pub struct App {
    simulation_controller_ui: SimulationControllerUI,
    client_uis: Vec<Box<dyn ClientUI>>,
}

impl App {
    /// Create a new application instance
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        simulation_controller_ui: SimulationControllerUI,
        client_uis: Vec<Box<dyn ClientUI>>,
    ) -> Self {
        Self {
            simulation_controller_ui,
            client_uis,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.simulation_controller_ui.show_ui(ctx, _frame);

        for client_ui in &mut self.client_uis {
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of(client_ui.get_viewport_id()),
                ViewportBuilder::default()
                    .with_title(format!("Client {}", client_ui.get_viewport_id()))
                    .with_inner_size([250.0, 150.0]),
                |child_ctx, class| {
                    assert!(class == egui::ViewportClass::Immediate);
                    egui::CentralPanel::default().show(child_ctx, |ui| {
                        client_ui.show_ui(child_ctx, _frame, ui);
                    });
                },
            );
        }
        ctx.request_repaint();
    }
}
