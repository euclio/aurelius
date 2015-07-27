# aurelius

[aurelius](https://github.com/euclio/aurelius) is a complete solution for
rendering and previewing markdown.

This crate provides a server that can render and update an HTML preview of
markdown without a client-side refresh. The server listens for both WebSocket
and HTTP connections on arbitrary ports. Upon receiving an HTTP request, the
server renders a page containing a markdown preview. Client-side JavaScript then
initiates a WebSocket connection which allows the server to push changes to the
client.

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

[vim-markdown-composer]: https://github.com/euclio/vim-markdown-composer
