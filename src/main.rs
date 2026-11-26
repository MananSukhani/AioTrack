//! Desktop entry point. The phone build uses `android_main` in the library.

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    use eframe::egui;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AioTrack")
            .with_inner_size([420.0, 900.0])
            .with_min_inner_size([360.0, 640.0]),
        ..Default::default()
    };
    let path = state_path();
    eframe::run_native(
        "AioTrack",
        options,
        Box::new(move |cc| Ok(Box::new(aiotrack::AioApp::new(cc, path)))),
    )
}

#[cfg(not(target_os = "android"))]
fn state_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|home| {
        let mut path = std::path::PathBuf::from(home);
        path.push(".local/share/aiotrack/state.json");
        path
    })
}

#[cfg(target_os = "android")]
fn main() {}
