//! Error types.

use std::io;

use url;

error_chain! {
    foreign_links {
        Io(io::Error) #[doc = "Error during IO."];
        UrlParse(url::ParseError) #[doc = "Error parsing a URL."];
    }
}
