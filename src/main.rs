use std::process::ExitCode;

use herdr_image_hints::adapters::config::DEFAULT_CONFIG;
use herdr_image_hints::app;

const USAGE: &str = "\
herdr-image-hints — one-key hints over image paths, images in a popup

Usage:
  herdr-image-hints start                    plugin action: open the hints overlay
  herdr-image-hints ui                       the overlay itself (run by Herdr)
  herdr-image-hints view                     the image popup (run by Herdr)
  herdr-image-hints open <image>             show an image in the popup (from a Herdr pane)
  herdr-image-hints scan [--width N] < dump  list the hints a screen dump would get
  herdr-image-hints default-config           print the commented default config.toml
  herdr-image-hints --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("start") => app::start(),
        Some("ui") => app::ui(),
        Some("view") => app::view(),
        Some("open") => app::open(&args[1..]),
        Some("scan") => app::scan(&args[1..]),
        Some("default-config") => {
            print!("{DEFAULT_CONFIG}");
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("herdr-image-hints {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("herdr-image-hints: {error}");
            ExitCode::FAILURE
        }
    }
}
