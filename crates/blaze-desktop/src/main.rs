mod app;

#[cfg(feature = "servo-host")]
#[path = "servo_host.rs"]
mod servo_host;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let first = args.next();

    match first.as_deref() {
        Some("--version") | Some("-V") => {
            println!("Blaze {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            println!("Blaze {}\n\nUsage:\n  blaze                 Open the workspace shell\n  blaze --version       Print the beta version\n  blaze --help          Show this help", env!("CARGO_PKG_VERSION"));
            #[cfg(feature = "servo-host")]
            println!("  blaze --servo-host [URL]  Open the bundled Servo browser mode");
            #[cfg(not(feature = "servo-host"))]
            println!("\nBuild with --features servo-host to enable the browser mode.");
            return Ok(());
        }
        #[cfg(feature = "servo-host")]
        Some("--servo-host") => return servo_host::run(),
        Some(unknown) => {
            return Err(format!("Unknown option: {unknown}. Use --help for usage.").into());
        }
        None => {}
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
