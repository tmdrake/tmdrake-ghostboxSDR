mod app;
mod audio;
#[cfg(windows)]
mod winusb;

use anyhow::Context;
use shackhack_sdr::SourceKind;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("shackhack=info".parse()?))
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--list") {
        return list_devices();
    }
    if args.iter().any(|a| a == "--setup-driver") {
        return setup_driver();
    }

    let synth = args.iter().any(|a| a == "--synth");
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

fn print_help() {
    println!(
        "\
ShackHack — RTL-SDR analog ghost box

  shackhack                 open the radio (needs WinUSB on the dongle)
  shackhack --synth         fake IQ, no dongle
  shackhack --list          USB / WinUSB status
  shackhack --setup-driver  bind WinUSB (Windows package, no kernel driver to build)
  shackhack --help
"
    );
}

fn list_devices() -> anyhow::Result<()> {
    println!("rs-rtl devices:");
    match shackhack_sdr::list_rtl_devices() {
        Ok(list) if list.is_empty() => println!("  (none — WinUSB may be missing on Interface 0)"),
        Ok(list) => {
            for line in list {
                println!("  {line}");
            }
        }
        Err(e) => println!("  {e}"),
    }
    println!();
    #[cfg(windows)]
    winusb::print_status()?;
    #[cfg(not(windows))]
    println!("WinUSB package is Windows-only. On Linux the stock rtl-sdr udev rules are enough.");
    Ok(())
}

fn setup_driver() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        winusb::setup()
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("--setup-driver is Windows-only (WinUSB bind via Zadig)")
    }
}
