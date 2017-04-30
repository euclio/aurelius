extern crate aurelius;

use aurelius::Server;
use aurelius::browser;

fn main() {
    let server = Server::new()
        .initial_markdown("<h1>Hello, world!</h1>")
        .start()
        .unwrap();

    let url = format!("http://localhost:{}", server.http_addr().unwrap().port());
    browser::open(&url).unwrap();

    loop {}
}
