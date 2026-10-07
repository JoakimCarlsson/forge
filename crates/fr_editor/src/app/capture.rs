//! The headless run: the same editor on an offscreen renderer, a few frames,
//! then a PNG of the last one.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use fr_render::{Capture, Renderer};

use super::editor::Editor;
use crate::options::EditorOptions;

/// The seconds each frame of a capture advances.
const FRAME_SECONDS: f32 = 1.0 / 60.0;

/// The fewest frames a capture runs: one to lay the dock out, one to draw it.
const MIN_FRAMES: u32 = 2;

/// Writes a captured frame as an 8 bit RGBA PNG, creating the folder.
///
/// # Errors
///
/// The message of the file that cannot be written.
fn write_png(path: &Path, capture: &Capture) -> Result<(), String> {
    if let Some(folder) = path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        fs::create_dir_all(folder)
            .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
    }
    let file =
        File::create(path).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), capture.width, capture.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    writer
        .write_image_data(&capture.rgba)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Runs the editor offscreen for the frames the options ask for and writes the
/// last one to the capture path.
///
/// # Errors
///
/// The message of the project, scene or entity that cannot be opened, the device
/// that cannot be created or the image that cannot be written.
pub fn run_capture(options: &EditorOptions) -> Result<(), String> {
    let path = options
        .capture
        .as_deref()
        .ok_or_else(|| "--capture needs a path".to_owned())?;
    let mut editor = Editor::start(options, false)?;
    let (width, height) = options.size;
    let physical = (
        (width as f32 * options.scale).round() as u32,
        (height as f32 * options.scale).round() as u32,
    );
    let renderer = Renderer::new_offscreen(physical.0, physical.1, options.scale)
        .map_err(|error| error.to_string())?;
    editor.attach_renderer(renderer);
    for _ in 0..options.frames.max(MIN_FRAMES) {
        editor.frame(FRAME_SECONDS);
    }
    let capture = editor.capture()?;
    write_png(path, &capture)
}
