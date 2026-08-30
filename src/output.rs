// Marker protocol for communicating with the shell function wrapper.
// The shell function captures stdout and dispatches actions based on these prefixes.
// All user-facing messages must go to stderr via the `info!` macro instead.

use std::path::Path;

pub(crate) const MARKER_NAMESPACE: &str = "__COPSY_";
pub(crate) const CD_MARKER: &str = "__COPSY_CD__";
pub(crate) const LAUNCH_MARKER: &str = "__COPSY_LAUNCH__";
pub(crate) const OPEN_MARKER: &str = "__COPSY_OPEN__";
pub(crate) const SETUP_MARKER: &str = "__COPSY_SETUP__";
#[cfg(test)]
pub(crate) const MARKERS: [&str; 4] = [CD_MARKER, LAUNCH_MARKER, OPEN_MARKER, SETUP_MARKER];

pub fn request_cd(path: &Path) {
    println!("{CD_MARKER}{}", path.display());
}

// LAUNCH markers are dispatched via case statement in the shell function (no eval)
pub fn request_launch(tool: &str, path: &Path) {
    println!("{LAUNCH_MARKER}{tool}\t{}", path.display());
}

// OPEN markers use eval in the shell function — only for user-provided --open commands
pub fn request_open(cmd: &str) {
    println!("{OPEN_MARKER}{cmd}");
}

pub fn request_setup(path: &Path) {
    println!("{SETUP_MARKER}{}", path.display());
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        eprintln!($($arg)*)
    };
}
