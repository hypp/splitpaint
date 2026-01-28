mod app;
mod canvas;
mod types;

use app::PixelArtApp;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1100.0, 800.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "SplitPaint",
        options,
        Box::new(|_cc| Box::<PixelArtApp>::default()),
    )
}