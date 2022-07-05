//! HTML rendering.

use std::error::Error;

mod command;
mod markdown;

pub use command::CommandRenderer;
pub use markdown::MarkdownRenderer;

/// HTML renderer implementation.
///
/// Implementors of this trait convert input into HTML.
pub trait Renderer {
    /// Renders input as HTML.
    ///
    /// The HTML should be written directly into the `html` buffer. The buffer will be reused
    /// between multiple calls to this method, with its capacity already reserved, so this function
    /// only needs to write the HTML.
    fn render(&self, input: &str, html: &mut String) -> Result<(), Box<dyn Error + Sync + Send>>;

    /// A hint for how many bytes the output will be.
    ///
    /// This hint should be cheap to compute and is not required to be accurate. However, accurate
    /// hints may improve performance by saving intermediate allocations when reserving capacity
    /// for the output buffer.
    fn size_hint(&self, input: &str) -> usize {
        input.len()
    }
}
