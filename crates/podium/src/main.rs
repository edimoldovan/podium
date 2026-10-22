use std::path::PathBuf;

use eframe::egui;
use podium::app::PodiumApp;
use podium::session::Session;
use podium::{claude, mcp};

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("mcp") {
        if let Err(e) = mcp::shim() {
            eprintln!("podium mcp: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let path = args.iter().find(|a| !a.starts_with('-')).map(|a| std::path::absolute(a).unwrap_or_else(|_| PathBuf::from(a)));

    // A window is open already: it takes the file and comes forward.
    if mcp::hand_off(path.as_deref()) {
        return Ok(());
    }
    std::thread::spawn(claude::register);

    let options = eframe::NativeOptions {
        // A Wayland window that is not on screen gets no frame callbacks, and
        // a vsynced swap waits for one: Claude's calls would wait with it.
        vsync: false,
        viewport: egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]).with_title("Podium").with_app_id("podium"),
        ..Default::default()
    };
    eframe::run_native(
        "podium",
        options,
        Box::new(move |cc| {
            omarchy_kit::fonts::install(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(omarchy_kit::theme::visuals(&omarchy_kit::theme::load()));
            let (session, error) = Session::start(path);
            let mut app = PodiumApp::new(session, true);
            if let Some(error) = error {
                app.status = error;
            }
            app.mcp = mcp::listen(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
}
