extern crate aurelius;
extern crate hyper;
extern crate url;

use aurelius::Server;
use hyper::Client;
use hyper::status::StatusCode;
use url::Url;

#[test]
fn change_working_directory() {
    let mut server = Server::new();

    // Try to find a resource outside of the working directory.
    let http_port = server.http_addr().unwrap().port();
    let mut resource_url = Url::parse(&format!("http://localhost:{}", http_port)).unwrap();
    resource_url.set_path("/file");

    let response = Client::new().get(resource_url.clone()).send().unwrap();
    assert_eq!(response.status, StatusCode::NotFound);

    // Change to a directory where the file exists
    server.change_working_directory("tests/assets");

    let response = Client::new().get(resource_url).send().unwrap();
    assert_eq!(response.status, StatusCode::Ok);
}
