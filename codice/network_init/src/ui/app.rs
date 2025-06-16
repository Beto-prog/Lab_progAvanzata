use common::client_ui::ClientUI;
use eframe::egui;
use simulation_controller::SimulationControllerUI;

/// Main application state
pub struct App {
    simulation_controller_ui: SimulationControllerUI,
    client_uis: Vec<Box<dyn ClientUI>>,
    selected_tab: usize,
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
            selected_tab: 0,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Network simulation");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Simulation Controller").clicked() {
                    self.selected_tab = 0;
                }
                for (i, client_ui) in self.client_uis.iter_mut().enumerate() {
                    if ui
                        .button(format!("Client {}", client_ui.get_viewport_id()))
                        .clicked()
                    {
                        self.selected_tab = i + 1;
                    }
                }
            });
            if self.selected_tab == 0 {
                self.simulation_controller_ui.show_ui(ctx, _frame, ui);
            } else {
                self.client_uis[self.selected_tab - 1].show_ui(_frame, ui);
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}
