//! Functions for interacting with browser processes.

use std::process::{Command, Child, Stdio};
use std::io::Result;

use url::Url;

/// Opens a browser window at the specified URL in a new process.
///
/// Returns an `io::Result` containing the child process.
///
/// This function uses platform-specific utilities to determine the user's default browser. The
/// following platforms are supported:
///
/// | Platform | Program    |
/// | -------- | ---------- |
/// | Linux    | `xdg-open` |
/// | OS X     | `open -g   |
/// | Windows  | `start`    |
///
/// # Panics
/// Panics if called on an unsupported operating system.
pub fn open(url: &str) -> Result<Child> {
    let (browser, args) = if cfg!(target_os = "linux") {
        ("xdg-open", vec![])
    } else if cfg!(target_os = "macos") {
        ("open", vec!["-g"])
    } else if cfg!(target_os = "windows") {
        // `start` requires an empty string as its first parameter.
        ("start", vec![""])
    } else {
        panic!("unsupported OS")
    };
    open_specific(url, &browser, &args)
}

/// Opens a specified browser in a new process.
///
/// The browser will be called with any supplied arguments in addition to the URL as an additional
/// argument.
///
/// Returns an `io::Result` containing the child process.
pub fn open_specific(url: &str, browser: &str, browser_args: &[&str]) -> Result<Child> {
    let url = Url::parse(url).unwrap();
    debug!("starting process '{:?}' with url {:?}", browser, url);

    Command::new(browser)
        .args(browser_args)
        .arg(url.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
