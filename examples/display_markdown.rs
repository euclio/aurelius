use aurelius::{browser, Config, Server};

fn main() {
    let server = Server::new_with_config(Config {
        initial_markdown: Some(String::from("# Hello world!")),
        ..Default::default()
    }).start()
        .unwrap();

    let url = format!("http://localhost:{}", server.http_addr().unwrap().port());
    browser::open(&url).unwrap();

    loop {}
}
