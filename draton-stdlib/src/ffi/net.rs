use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Method;

use crate::{NetError, Response};

fn client() -> Result<reqwest::blocking::Client, NetError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| NetError::new(error.to_string()))
}

fn send(method: Method, url: &str, body: Option<&str>) -> Result<Response, NetError> {
    let client = client()?;
    let mut request = client.request(method, url);
    if let Some(body) = body {
        request = request.body(body.to_string());
    }
    let response = request
        .send()
        .map_err(|error| NetError::new(error.to_string()))?;
    let status = response.status().as_u16() as i64;
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let body = response
        .text()
        .map_err(|error| NetError::new(error.to_string()))?;
    Ok(Response::new(status, body, headers))
}

/// Performs a blocking HTTP GET request.
pub fn get(url: impl AsRef<str>) -> Result<Response, NetError> {
    send(Method::GET, url.as_ref(), None)
}

/// Performs a blocking HTTP POST request.
pub fn post(url: impl AsRef<str>, body: impl AsRef<str>) -> Result<Response, NetError> {
    send(Method::POST, url.as_ref(), Some(body.as_ref()))
}

/// Performs a blocking HTTP PUT request.
pub fn put(url: impl AsRef<str>, body: impl AsRef<str>) -> Result<Response, NetError> {
    send(Method::PUT, url.as_ref(), Some(body.as_ref()))
}

/// Performs a blocking HTTP DELETE request.
pub fn delete_req(url: impl AsRef<str>) -> Result<Response, NetError> {
    send(Method::DELETE, url.as_ref(), None)
}
