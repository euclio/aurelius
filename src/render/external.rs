use std::cell::RefCell;
use std::io::{self, prelude::*};
use std::process::{Command, Stdio};

use super::Renderer;

/// Renderer that uses an external command to render input.
///
/// [`MarkdownRenderer`](crate::render::MarkdownRenderer) uses an extremely fast, in-memory parser
/// that is sufficient for most use-cases. However, this renderer may be useful if your markdown
/// requires features unsupported by [`pulldown_cmark`].
///
/// # Example
///
/// Creating an external renderer that uses [pandoc](https://pandoc.org/) to render markdown:
///
/// ```no_run
/// use std::process::Command;
/// use aurelius::CommandRenderer;
///
/// let mut pandoc = Command::new("pandoc");
/// pandoc.args(&["-f", "markdown", "-t", "html"]);
///
/// CommandRenderer::new(pandoc);
/// ```
#[derive(Debug)]
pub struct CommandRenderer {
    command: RefCell<Command>,
}

impl CommandRenderer {
    /// Create a new external command renderer that will spawn processes using the given `command`.
    ///
    /// The provided [`Command`] should expect markdown input on stdin and print HTML on stdout.
    pub fn new(mut command: Command) -> CommandRenderer {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        CommandRenderer {
            command: RefCell::new(command),
        }
    }
}

impl Renderer for CommandRenderer {
    type Error = io::Error;

    fn render(&self, input: &str, html: &mut String) -> Result<(), Self::Error> {
        let child = self.command.borrow_mut().spawn()?;

        child.stdin.unwrap().write_all(input.as_bytes())?;

        child.stdout.unwrap().read_to_string(html)?;

        Ok(())
    }
}
