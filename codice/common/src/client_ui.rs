use eframe::egui;

pub trait ClientUI {
    fn show_ui(&mut self, _frame: &mut eframe::Frame, ui: &mut egui::Ui);
    fn get_viewport_id(&self) -> u64;
}
