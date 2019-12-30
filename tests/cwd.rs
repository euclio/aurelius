use url::Url;
use reqwest::StatusCode;

use aurelius::Server;

#[test]
fn change_working_directory() {
    let mut listening = Server::new().start().unwrap();

    // Try to find a resource outside of the working directory.
    let http_port = listening.http_addr().unwrap().port();
    let mut resource_url = Url::parse(&format!("http://localhost:{}", http_port)).unwrap();
    resource_url.set_path("/file");

    let response = reqwest::get(resource_url.clone()).unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Change to a directory where the file exists
    listening.change_working_directory("tests/assets");

    let response = reqwest::get(resource_url).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
