//! The `forge_editor` binary.

use std::process::ExitCode;

use fr_editor::app::capture::run_capture;
use fr_editor::app::host::run_window;
use fr_editor::options::{HELP, parse_editor_options};

/// Parses the command line and runs the editor, in a window or offscreen into
/// a PNG; a failure is `forge: <message>` on stderr and a nonzero exit.
fn main() -> ExitCode {
    let options = match parse_editor_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("forge: {error}");
            return ExitCode::FAILURE;
        }
    };
    if options.help {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let outcome = if options.capture.is_some() {
        run_capture(&options)
    } else {
        run_window(&options)
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            ExitCode::FAILURE
        }
    }
}
