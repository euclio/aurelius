# aurelius

[aurelius](https://github.com/euclio/aurelius) is a complete solution for
rendering and previewing markdown.

[![Build Status](https://travis-ci.org/euclio/aurelius.svg)](https://travis-ci.org/euclio/aurelius)
[![crates.io](http://meritbadge.herokuapp.com/aurelius)](https://crates.io/crates/aurelius)

This crate provides a server that can render and update an HTML preview of
markdown without a client-side refresh. The server listens for both WebSocket
and HTTP connections on arbitrary ports. Upon receiving an HTTP request, the
server renders a page containing a markdown preview. Client-side JavaScript then
initiates a WebSocket connection which allows the server to push changes to the
client.

Full documentation may be found [here](https://euclio.github.io/aurelius).

This crate was designed to power [vim-markdown-composer], a markdown preview
plugin for [Neovim](http://neovim.io), but it may be used to implement similar
plugins for any editor. See [vim-markdown-composer] for a usage example.

aurelius follows stable Rust. However, the API currently unstable and may change
without warning.

# Acknowledgments
This crate is inspired by suan's
[instant-markdown-d](https://github.com/suan/instant-markdown-d).

# Why the name?
"Aurelius" is a Roman *gens* (family name) shared by many famous Romans,
including emperor Marcus Aurelius, one of the "Five Good Emperors." The gens
itself originates from the Latin *aureus* meaning "golden." Also, tell me that
"Markdown Aurelius" isn't a great pun.

<cite>[Aurelia (gens) on Wikipedia](https://en.wikipedia.org/wiki/Aurelia_(gens))</cite>.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

[vim-markdown-composer]: https://github.com/euclio/vim-markdown-composer
