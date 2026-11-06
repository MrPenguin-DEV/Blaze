mod app;

#[cfg(feature = "servo-host")]
#[path = "servo_host.rs"]
mod servo_host;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "servo-host")]
    if std::env::args().nth(1).as_deref() == Some("--servo-host") {
        return servo_host::run();
    }

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
    )?;

    Ok(())
}
