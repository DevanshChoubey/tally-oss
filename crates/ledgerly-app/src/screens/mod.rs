//! Screens, one module per card.

pub mod ai_settings;
pub mod ask;
pub mod assist;
pub mod audit;
pub mod bank;
pub mod gst;
pub mod home;
pub mod import;
pub mod item_invoice;
pub mod keymap_map;
pub mod masters;
pub mod purchase_bill;
pub mod quick_entry;
pub mod report_view;
pub mod review;
pub mod setup;
pub mod voucher_entry;
pub mod wire;

/// Draws a stub screen (a mock screen the app does not build yet): its title, then one line in the
/// app's own "not supported yet" words that names the mock states the screen will implement. The
/// stub shows no figure and no sample data.
pub fn draw_stub(ui: &mut egui::Ui, title: &str, states: &str) {
    ui.heading(title);
    let line = wire::unsupported_message(
        &format!("the {title} screen"),
        &format!("it is not built yet and will implement the mock states {states}"),
    );
    ui.label(line);
}
