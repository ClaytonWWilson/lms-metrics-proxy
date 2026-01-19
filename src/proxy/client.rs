use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;

pub type HttpClient = Client<hyper_tls::HttpsConnector<HttpConnector>, String>;

pub fn create_client() -> HttpClient {
    let https = hyper_tls::HttpsConnector::new();
    Client::builder(TokioExecutor::new()).build(https)
}

pub async fn forward_request(
    client: &HttpClient,
    mut req: Request<String>,
    lm_studio_url: &str,
) -> Result<Response<Incoming>, crate::error::ProxyError> {
    // Build the full URL to LM Studio
    let path = req.uri().path();
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
    let target_url = format!("{}{}{}", lm_studio_url, path, query);

    // Parse the target URL
    let target_uri: hyper::Uri = target_url
        .parse()
        .map_err(|e| crate::error::ProxyError::Http(format!("Invalid URL: {}", e)))?;

    // Update the Host header to match the target domain
    // This is critical for reverse proxies to route correctly
    if let Some(authority) = target_uri.authority() {
        req.headers_mut().insert(
            hyper::header::HOST,
            authority.as_str().parse().map_err(|e| {
                crate::error::ProxyError::Http(format!("Invalid host header: {}", e))
            })?,
        );
    }

    *req.uri_mut() = target_uri;

    // Forward the request to LM Studio
    client
        .request(req)
        .await
        .map_err(|e| crate::error::ProxyError::LmStudioConnection(e.to_string()))
}
