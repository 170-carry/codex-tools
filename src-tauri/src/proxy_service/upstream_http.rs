use reqwest::{Client, Request, Response};
use std::error::Error;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct UpstreamHttp(Arc<RwLock<Client>>);

impl From<Client> for UpstreamHttp {
    fn from(client: Client) -> Self {
        Self(Arc::new(RwLock::new(client)))
    }
}

impl UpstreamHttp {
    pub(super) fn new() -> Result<Self, String> {
        Self::build_client().map(Self::from).map_err(describe)
    }

    fn build_client() -> reqwest::Result<Client> {
        Client::builder()
            .user_agent("codex-tools-proxy/0.1")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(
                super::DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS,
            ))
            .build()
    }

    pub(super) fn client(&self) -> Client {
        self.0.read().unwrap().clone()
    }

    pub(super) async fn send(&self, request: Request) -> Result<Response, String> {
        self.send_with(request, Self::build_client)
            .await
            .map_err(describe)
    }

    async fn send_with(
        &self,
        request: Request,
        reconnect: impl FnOnce() -> reqwest::Result<Client>,
    ) -> reqwest::Result<Response> {
        // All upstream bodies are buffered JSON. A connect failure occurs before submission.
        let first = request.try_clone().expect("buffered upstream request");
        match self.client().execute(first).await {
            Err(error) if error.is_connect() => {
                let fresh = reconnect()?;
                *self.0.write().unwrap() = fresh.clone();
                fresh.execute(request).await
            }
            Err(error) if error.is_request() => {
                // The server may have received this POST. Renew the pool for the next request.
                if let Ok(fresh) = reconnect() {
                    *self.0.write().unwrap() = fresh;
                }
                Err(error)
            }
            result => result,
        }
    }
}

fn describe(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(source) = cause {
        message.push_str(": ");
        message.push_str(&source.to_string());
        cause = source.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Router};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn refused_connection_rebuilds_and_reuses_client() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/responses", listener.local_addr().unwrap());
        let server = tokio::spawn(async {
            axum::serve(
                listener,
                Router::new().route("/responses", post(|| async { "ok" })),
            )
            .await
            .unwrap()
        });
        let unused = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy =
            reqwest::Proxy::all(format!("http://{}", unused.local_addr().unwrap())).unwrap();
        drop(unused);
        let http = UpstreamHttp::from(Client::builder().proxy(proxy).build().unwrap());
        let request = || http.client().post(&url).body("test").build().unwrap();
        let response = http
            .send_with(request(), || Client::builder().no_proxy().build())
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "ok");
        assert!(http
            .send_with(request(), || panic!("client was not retained"))
            .await
            .unwrap()
            .status()
            .is_success());
        server.abort();
    }
}
