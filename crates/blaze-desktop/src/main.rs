mod app;

#[cfg(feature = "servo-browser")]
mod servo_embed;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("Blaze {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            println!("Blaze {}\n\nUsage:\n  blaze                 Open the Blaze workspace\n  blaze --version       Print the beta version\n  blaze --help          Show this help", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(unknown) => return Err(format!("Unknown option: {unknown}. Use --help for usage.").into()),
        None => {}
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Blaze"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Blaze",
        options,
        Box::new(|creation_context| Ok(Box::new(app::BlazeApp::new(creation_context)))),
    )?;

    Ok(())
}
