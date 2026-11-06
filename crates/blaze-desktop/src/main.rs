mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Blaze"),
        ..Default::default()
    };

    eframe::run_native(
        "Blaze",
        options,
        Box::new(|_creation_context| Ok(Box::new(app::BlazeApp::default()))),
    )
}
