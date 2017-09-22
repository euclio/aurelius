//! Functions for interacting with browser processes.

use std::process::{Command, Child, Stdio};

use url::Url;

use errors::*;

/// Opens a browser window at the specified URL in a new process.
///
/// This function uses platform-specific utilities to determine the user's default browser. The
/// following platforms are supported:
///
/// | Platform | Program    |
/// | -------- | ---------- |
/// | Linux    | `xdg-open` |
/// | OS X     | `open -g`  |
/// | Windows  | `explorer` |
///
/// # Panics
/// Panics if called on an unsupported operating system.
pub fn open(url: &str) -> Result<Child> {
    let command = if cfg!(target_os = "linux") {
        Command::new("xdg-open")
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg("-g");
        command
    } else if cfg!(target_os = "windows") {
        Command::new("explorer")
    } else {
        bail!("unsupported OS: set a browser to use explicitly")
    };
    open_specific(url, command)
}

/// Opens a specified browser in a new process.
///
/// The browser will be called with any supplied arguments in addition to the URL as an additional
/// argument.
pub fn open_specific(url: &str, mut browser: Command) -> Result<Child> {
    let url = Url::parse(url).chain_err(
        || "error parsing URL for the browser",
    )?;
    debug!("starting process '{:?}' with url {}", browser, url);

    let child = browser
        .arg(url.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .chain_err(|| "error executing browser")?;

    Ok(child)
}
