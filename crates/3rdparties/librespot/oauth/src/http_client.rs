//! Adapt OAuth2's HTTP 1 request contract to the shared Reqwest 0.13 client.

use oauth2::{HttpRequest, HttpResponse};

#[cfg(feature = "rustls-tls-webpki-roots")]
fn webpki_tls_config() -> rustls::ClientConfig {
    let roots: rustls::RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("AWS-LC supports the safe TLS protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth()
}

pub(super) fn blocking_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    let builder = reqwest::blocking::Client::builder();
    #[cfg(feature = "rustls-tls-webpki-roots")]
    let builder = builder.tls_backend_preconfigured(webpki_tls_config());
    builder.build()
}

pub(super) fn asynchronous_client() -> Result<reqwest::Client, reqwest::Error> {
    let builder = reqwest::Client::builder();
    #[cfg(feature = "rustls-tls-webpki-roots")]
    let builder = builder.tls_backend_preconfigured(webpki_tls_config());
    builder.build()
}

pub(super) fn blocking(
    client: &reqwest::blocking::Client,
    request: HttpRequest,
) -> Result<HttpResponse, reqwest::Error> {
    let (parts, body) = request.into_parts();
    let response = client
        .request(parts.method, parts.uri.to_string())
        .headers(parts.headers)
        .body(body)
        .send()?;
    let status = response.status();
    let headers = response.headers().clone();
    let mut result = http::Response::new(response.bytes()?.to_vec());
    *result.status_mut() = status;
    *result.headers_mut() = headers;
    Ok(result)
}

pub(super) async fn asynchronous(
    client: &reqwest::Client,
    request: HttpRequest,
) -> Result<HttpResponse, reqwest::Error> {
    let (parts, body) = request.into_parts();
    let response = client
        .request(parts.method, parts.uri.to_string())
        .headers(parts.headers)
        .body(body)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let mut result = http::Response::new(response.bytes().await?.to_vec());
    *result.status_mut() = status;
    *result.headers_mut() = headers;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn selected_tls_backend_builds_blocking_and_async_clients() {
        blocking_client().expect("configured blocking OAuth client");
        asynchronous_client().expect("configured async OAuth client");
        #[cfg(feature = "rustls-tls-webpki-roots")]
        assert!(!webpki_roots::TLS_SERVER_ROOTS.is_empty());
    }

    fn mock_token_endpoint() -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind mock OAuth endpoint");
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept mock OAuth request");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut bytes = [0; 1024];
            loop {
                let n = stream.read(&mut bytes).expect("read mock OAuth request");
                assert!(n > 0, "OAuth request ended before its body");
                request.extend_from_slice(&bytes[..n]);
                if request.windows(3).any(|window| window == b"a=b") {
                    break;
                }
            }
            stream.write_all(b"HTTP/1.1 201 Created\r\nx-token-test: preserved\r\ncontent-length: 2\r\n\r\nok")
                .expect("write mock OAuth response");
            request
        });
        (url, task)
    }

    fn request(url: &str) -> HttpRequest {
        http::Request::builder()
            .method(http::Method::POST)
            .uri(url)
            .header(
                http::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header("x-client-test", "preserved")
            .body(b"a=b".to_vec())
            .unwrap()
    }

    fn assert_exchange(response: HttpResponse, recorded: Vec<u8>) {
        assert_eq!(response.status(), http::StatusCode::CREATED);
        assert_eq!(response.headers()["x-token-test"], "preserved");
        assert_eq!(response.body(), b"ok");
        let text = String::from_utf8(recorded).unwrap();
        assert!(text.starts_with("POST /token HTTP/1.1"));
        assert!(
            text.to_ascii_lowercase()
                .contains("x-client-test: preserved")
        );
        assert!(text.ends_with("a=b"));
    }

    #[test]
    fn blocking_bridge_preserves_oauth_request_and_response() {
        let (url, task) = mock_token_endpoint();
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap();
        let response = blocking(&client, request(&url)).unwrap();
        assert_exchange(response, task.join().unwrap());
    }

    #[tokio::test]
    async fn async_bridge_preserves_oauth_request_and_response() {
        let (url, task) = mock_token_endpoint();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = asynchronous(&client, request(&url)).await.unwrap();
        assert_exchange(response, task.join().unwrap());
    }
}
