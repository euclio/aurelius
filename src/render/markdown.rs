use std::convert::Infallible;

use pulldown_cmark::{html, Options, Parser};

use super::Renderer;

/// Markdown renderer that uses [`pulldown_cmark`] as the backend.
#[derive(Debug)]
pub struct Markdown {
    options: Options,
}

impl Markdown {
    /// Create a new instance of the renderer.
    pub fn new() -> Markdown {
        Markdown {
            options: Options::ENABLE_FOOTNOTES
                | Options::ENABLE_TABLES
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS,
        }
    }
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for Markdown {
    type Error = Infallible;

    fn render(&self, markdown: &str, html: &mut String) -> Result<(), Self::Error> {
        let parser = Parser::new_ext(markdown, self.options);

        html::push_html(html, parser);

        Ok(())
    }

    fn size_hint(&self, input: &str) -> usize {
        input.len() * 3 / 2
    }
}
