//! The image popup: decodes the image and draws it centered with the Kitty
//! graphics protocol, which Herdr forwards to a capable outer terminal. Any
//! key closes the popup.

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, WindowSize};
use crossterm::{cursor, execute, queue, style};
use image::DynamicImage;
use image::imageops::FilterType;

/// Kitty graphics payloads go out in chunks of at most this many base64 bytes.
const CHUNK: usize = 4096;
/// Cell size assumed when the terminal reports no pixel size.
const FALLBACK_CELL: (f64, f64) = (10.0, 20.0);

/// Shows the image at `path` until a key is pressed.
pub fn show(path: &Path) -> io::Result<()> {
    let image = decode(path);
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    let result = run(&mut out, path, &image);
    let _ = write!(out, "\x1b_Ga=d,d=A,q=2\x1b\\");
    let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    result
}

fn decode(path: &Path) -> Result<DynamicImage, String> {
    image::ImageReader::open(path)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .decode()
        .map_err(|error| error.to_string())
}

fn run(out: &mut impl Write, path: &Path, image: &Result<DynamicImage, String>) -> io::Result<()> {
    draw(out, path, image, settled_size()?)?;
    loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => return Ok(()),
            Event::Resize(..) => draw(out, path, image, settled_size()?)?,
            _ => {}
        }
    }
}

/// The window size once it stops changing. A popup can start at one size
/// and be resized right after it opens; drawing at the first size would
/// place the image wrongly.
fn settled_size() -> io::Result<WindowSize> {
    let deadline = Instant::now() + Duration::from_millis(400);
    let mut last = terminal::window_size()?;
    loop {
        std::thread::sleep(Duration::from_millis(40));
        let now = terminal::window_size()?;
        if same_size(&now, &last) || Instant::now() >= deadline {
            return Ok(now);
        }
        last = now;
    }
}

fn same_size(a: &WindowSize, b: &WindowSize) -> bool {
    (a.columns, a.rows, a.width, a.height) == (b.columns, b.rows, b.width, b.height)
}

fn draw(
    out: &mut impl Write,
    path: &Path,
    image: &Result<DynamicImage, String>,
    size: WindowSize,
) -> io::Result<()> {
    write!(out, "\x1b_Ga=d,d=A,q=2\x1b\\")?;
    queue!(out, terminal::Clear(terminal::ClearType::All))?;
    let area = (size.columns, size.rows.saturating_sub(1));
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let footer = match image {
        Ok(image) => {
            let cell = cell_size(&size);
            let placement = fit((image.width(), image.height()), cell, area);
            draw_image(out, image, cell, placement)?;
            format!(
                "{name}  {}×{}  ·  any key closes",
                image.width(),
                image.height()
            )
        }
        Err(error) => {
            let message = format!("cannot show {name}: {error}");
            let x = area.0.saturating_sub(width(&message)) / 2;
            queue!(out, cursor::MoveTo(x, area.1 / 2), style::Print(&message))?;
            "any key closes".to_string()
        }
    };
    let footer: String = footer.chars().take(usize::from(size.columns)).collect();
    let x = size.columns.saturating_sub(width(&footer)) / 2;
    queue!(
        out,
        cursor::MoveTo(x, size.rows.saturating_sub(1)),
        style::SetAttribute(style::Attribute::Dim),
        style::Print(footer),
        style::SetAttribute(style::Attribute::Reset)
    )?;
    out.flush()
}

fn width(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

/// Pixels per cell, from the size the terminal reports or a common guess.
fn cell_size(size: &WindowSize) -> (f64, f64) {
    if size.columns == 0 || size.rows == 0 || size.width == 0 || size.height == 0 {
        return FALLBACK_CELL;
    }
    (
        f64::from(size.width) / f64::from(size.columns),
        f64::from(size.height) / f64::from(size.rows),
    )
}

/// Where the image goes, in cells: column, row, width, height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub col: u16,
    pub row: u16,
    pub cols: u16,
    pub rows: u16,
}

/// Scales an image of `pixels` to the largest size that fits `area` (in
/// cells) with its aspect ratio kept, and centers it.
pub fn fit(pixels: (u32, u32), cell: (f64, f64), area: (u16, u16)) -> Placement {
    let natural_cols = (f64::from(pixels.0) / cell.0).max(f64::MIN_POSITIVE);
    let natural_rows = (f64::from(pixels.1) / cell.1).max(f64::MIN_POSITIVE);
    let scale = (f64::from(area.0) / natural_cols).min(f64::from(area.1) / natural_rows);
    let cols = ((natural_cols * scale).round() as u16).clamp(1, area.0.max(1));
    let rows = ((natural_rows * scale).round() as u16).clamp(1, area.1.max(1));
    Placement {
        col: area.0.saturating_sub(cols) / 2,
        row: area.1.saturating_sub(rows) / 2,
        cols,
        rows,
    }
}

fn draw_image(
    out: &mut impl Write,
    image: &DynamicImage,
    cell: (f64, f64),
    placement: Placement,
) -> io::Result<()> {
    // Send no more pixels than the placement shows, at twice the cell size
    // guess so a wrong guess does not cost sharpness.
    let max_w = (f64::from(placement.cols) * cell.0 * 2.0) as u32;
    let max_h = (f64::from(placement.rows) * cell.1 * 2.0) as u32;
    let scaled;
    let image = if image.width() > max_w || image.height() > max_h {
        scaled = image.resize(max_w.max(1), max_h.max(1), FilterType::Triangle);
        &scaled
    } else {
        image
    };
    let mut png = Vec::new();
    image
        .write_to(&mut io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(io::Error::other)?;
    let encoded = STANDARD.encode(&png);
    queue!(out, cursor::MoveTo(placement.col, placement.row))?;
    write_chunks(out, &encoded, placement.cols, placement.rows)
}

/// Transmits and places a PNG in one go (`a=T`), scaled to `cols`×`rows`
/// cells, without moving the cursor (`C=1`) and without replies (`q=2`).
fn write_chunks(out: &mut impl Write, encoded: &str, cols: u16, rows: u16) -> io::Result<()> {
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK).collect();
    let last = chunks.len().saturating_sub(1);
    for (index, chunk) in chunks.iter().enumerate() {
        let more = u8::from(index != last);
        if index == 0 {
            write!(out, "\x1b_Ga=T,f=100,q=2,C=1,c={cols},r={rows},m={more};")?;
        } else {
            write!(out, "\x1b_Gm={more};")?;
        }
        out.write_all(chunk)?;
        write!(out, "\x1b\\")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_image_fills_the_width_and_is_centered_vertically() {
        let placement = fit((2000, 500), (10.0, 20.0), (100, 40));
        assert_eq!(
            placement,
            Placement {
                col: 0,
                row: 13,
                cols: 100,
                rows: 13
            }
        );
    }

    #[test]
    fn a_tall_image_fills_the_height_and_is_centered_horizontally() {
        let placement = fit((500, 2000), (10.0, 20.0), (100, 40));
        assert_eq!(placement.rows, 40);
        assert_eq!(placement.cols, 20);
        assert_eq!(placement.col, 40);
    }

    #[test]
    fn a_small_image_is_scaled_up_to_the_area() {
        let placement = fit((16, 16), (10.0, 20.0), (100, 40));
        assert_eq!((placement.cols, placement.rows), (80, 40));
    }

    #[test]
    fn a_tiny_area_still_gets_one_cell() {
        let placement = fit((100, 100), (10.0, 20.0), (0, 0));
        assert_eq!((placement.cols, placement.rows), (1, 1));
    }

    #[test]
    fn the_payload_is_chunked_with_the_placement_on_the_first_chunk() {
        let mut out = Vec::new();
        let payload = "A".repeat(CHUNK + 10);
        write_chunks(&mut out, &payload, 30, 12).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("\x1b_Ga=T,f=100,q=2,C=1,c=30,r=12,m=1;"));
        assert!(text.contains("\x1b\\\x1b_Gm=0;AAAAAAAAAA\x1b\\"));
        assert_eq!(text.matches("\x1b_G").count(), 2);
    }

    #[test]
    fn the_cell_size_comes_from_the_reported_pixels_or_the_guess() {
        let reported = WindowSize {
            rows: 40,
            columns: 100,
            width: 900,
            height: 800,
        };
        assert_eq!(cell_size(&reported), (9.0, 20.0));
        let unknown = WindowSize {
            width: 0,
            height: 0,
            ..reported
        };
        assert_eq!(cell_size(&unknown), FALLBACK_CELL);
    }
}
