mod external;
mod markdown;

pub use external::ExternalCommand;
pub use markdown::Markdown;

/// Markdown renderer implementation.
///
/// Implementors of this trait convert markdown into HTML.
pub trait Renderer {
    /// Potential errors returned by rendering. If rendering is infallible (markdown can always
    /// produce HTML from its input), this type can be set to [`std::convert::Infallible`].
    type Error;

    /// Renders markdown as HTML.
    ///
    /// The HTML should be written directly into the `html` buffer.
    fn render(&self, input: &str, html: &mut String) -> Result<(), Self::Error>;

    /// A hint for how many bytes the output will be.
    ///
    /// This hint should be cheap to compute and is not required to be accurate. However, accurate
    /// hints may improve performance by saving intermediate allocations.
    fn size_hint(&self, input: &str) -> usize {
        input.len()
    }
}
