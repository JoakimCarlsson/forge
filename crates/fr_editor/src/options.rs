//! The command line of the editor binary: the project to open and the options
//! that render a frame offscreen into a PNG without a window.

use std::path::PathBuf;

/// The frames a capture runs when `--frames` is absent.
pub const DEFAULT_CAPTURE_FRAMES: u32 = 6;

/// The size of a capture when `--size` is absent.
pub const DEFAULT_CAPTURE_SIZE: (u32, u32) = (1600, 900);

/// The text printed for `-h` and `--help`.
pub const HELP: &str = "Usage: forge_editor [PROJECT] [OPTIONS]

  PROJECT, --project <dir>   the project directory to open
  --picker                   show the project picker instead of opening a project
  --scene <path>             the scene to open, relative to the asset root or absolute
  --select <name>            the entity to select once the scene is open
  --tool <tool>              select, move, rotate or scale
  --show <dialog>            picker or none
  --capture <png>            render offscreen into a PNG and exit
  --frames <n>               frames to run before the capture (default 6)
  --size <W>x<H>             size of the capture in pixels (default 1600x900)
  --scale <f>                display scale of the capture (default 1)
  -h, --help                 print this text
";

/// The transform tool the editor starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolChoice {
    /// Pick entities.
    #[default]
    Select,
    /// Translate the selection.
    Move,
    /// Rotate the selection.
    Rotate,
    /// Scale the selection.
    Scale,
}

impl ToolChoice {
    /// The tool named by `--tool`.
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "select" => Some(Self::Select),
            "move" => Some(Self::Move),
            "rotate" => Some(Self::Rotate),
            "scale" => Some(Self::Scale),
            _ => None,
        }
    }
}

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorOptions {
    /// The project directory to open; none opens the last project or the picker.
    pub project: Option<PathBuf>,
    /// Whether to show the project picker even when a project could be opened.
    pub picker: bool,
    /// The scene to open, relative to the asset root or absolute.
    pub scene: Option<PathBuf>,
    /// The name of the entity to select once the scene is open.
    pub select: Option<String>,
    /// The tool to start with.
    pub tool: ToolChoice,
    /// Where to write a screenshot of an offscreen run; none opens a window.
    pub capture: Option<PathBuf>,
    /// How many frames an offscreen run draws before the screenshot.
    pub frames: u32,
    /// The size of the offscreen target in pixels.
    pub size: (u32, u32),
    /// The display scale factor of an offscreen run.
    pub scale: f32,
    /// Whether `-h` or `--help` was given.
    pub help: bool,
}

impl Default for EditorOptions {
    /// No project, a window, and the capture defaults.
    fn default() -> Self {
        Self {
            project: None,
            picker: false,
            scene: None,
            select: None,
            tool: ToolChoice::default(),
            capture: None,
            frames: DEFAULT_CAPTURE_FRAMES,
            size: DEFAULT_CAPTURE_SIZE,
            scale: 1.0,
            help: false,
        }
    }
}

/// Parses `WIDTHxHEIGHT` with both sides above zero.
fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once(['x', 'X'])?;
    let width: u32 = width.trim().parse().ok()?;
    let height: u32 = height.trim().parse().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

/// Parses the value of a flag with `parse`.
///
/// # Errors
///
/// A message naming the flag when the value is missing or `parse` rejects it.
fn parse_value<T>(
    flag: &str,
    value: Option<String>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<T, String> {
    let value = value.ok_or_else(|| format!("{flag} needs a value"))?;
    parse(&value).ok_or_else(|| format!("invalid value `{value}` for {flag}"))
}

/// Parses a flag's value as text.
///
/// # Errors
///
/// A message naming the flag when the value is missing.
fn text_value(flag: &str, value: Option<String>) -> Result<String, String> {
    parse_value(flag, value, |text| Some(text.to_owned()))
}

/// Parses `--show`.
fn apply_show(options: &mut EditorOptions, value: Option<String>) -> Result<(), String> {
    options.picker = parse_value("--show", value, |text| match text {
        "picker" => Some(true),
        "none" => Some(false),
        _ => None,
    })?;
    Ok(())
}

/// Applies one flag, taking its value from `arguments` when it needs one.
///
/// # Errors
///
/// A message naming the flag when it is unknown or its value is wrong.
fn apply_flag(
    options: &mut EditorOptions,
    flag: &str,
    arguments: &mut impl Iterator<Item = String>,
) -> Result<(), String> {
    match flag {
        "-h" | "--help" => options.help = true,
        "--picker" => options.picker = true,
        "--project" => options.project = Some(text_value(flag, arguments.next())?.into()),
        "--scene" => options.scene = Some(text_value(flag, arguments.next())?.into()),
        "--select" => options.select = Some(text_value(flag, arguments.next())?),
        "--capture" => options.capture = Some(text_value(flag, arguments.next())?.into()),
        "--tool" => options.tool = parse_value(flag, arguments.next(), ToolChoice::from_name)?,
        "--frames" => {
            options.frames = parse_value(flag, arguments.next(), |text| text.parse().ok())?;
        }
        "--size" => options.size = parse_value(flag, arguments.next(), parse_size)?,
        "--scale" => {
            options.scale = parse_value(flag, arguments.next(), |text| {
                text.parse::<f32>()
                    .ok()
                    .filter(|scale| scale.is_finite() && *scale > 0.0)
            })?;
        }
        "--show" => apply_show(options, arguments.next())?,
        _ => return Err(format!("unknown option {flag}")),
    }
    Ok(())
}

/// Parses the arguments after the program name. The first argument that does
/// not start with a dash is the project, like `--project`.
///
/// # Errors
///
/// A message naming the flag that is unknown or has a missing or invalid
/// value, or the extra bare argument.
pub fn parse_editor_options(
    arguments: impl IntoIterator<Item = String>,
) -> Result<EditorOptions, String> {
    let mut options = EditorOptions::default();
    let mut arguments = arguments.into_iter();
    let mut bare_seen = false;
    while let Some(argument) = arguments.next() {
        if argument.starts_with('-') && argument.len() > 1 {
            apply_flag(&mut options, &argument, &mut arguments)?;
        } else if bare_seen {
            return Err(format!("unexpected argument {argument}"));
        } else {
            bare_seen = true;
            options.project = Some(argument.into());
        }
    }
    Ok(options)
}
