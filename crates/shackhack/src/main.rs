mod app;
mod audio;

use anyhow::Context;
use shackhack_sdr::SourceKind;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("shackhack=info".parse()?))
        .init();

    let synth = std::env::args().any(|a| a == "--synth");
    let kind = if synth {
        SourceKind::Synth
    } else {
        SourceKind::Rtl
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("ShackHack — ghost box SDR"),
        ..Default::default()
    };

    eframe::run_native(
        "ShackHack",
        options,
        Box::new(move |cc| Ok(Box::new(app::ShackHackApp::new(cc, kind)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
    .context("ui")
}
