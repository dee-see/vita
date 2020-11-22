use crate::error::{Error, Result};
use crate::IntoSubdomain;
use reqwest::Client;
use serde_json::value::Value;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{info, trace, warn};

struct AnubisResult {
    results: Value,
}

impl AnubisResult {
    fn new(results: Value) -> Self {
        Self { results }
    }
}

impl IntoSubdomain for AnubisResult {
    fn subdomains(&self) -> Vec<String> {
        match self.results.as_array() {
            Some(array) => array.iter().map(|s| s.to_string()).collect(),
            None => Vec::new(),
        }
    }
}

fn build_url(host: &str) -> String {
    format!("https://jldc.me/anubis/subdomains/{}", host)
}

//TODO: `Result` should be std::result::Result<(), Error::source_error>
// the `run` should be a method on a struct `AnubisDB` or a trait across the whole project?
pub async fn run(client: Client, host: Arc<String>, mut sender: Sender<Vec<String>>) -> Result<()> {
    trace!("fetching data from anubisdb for: {}", &host);
    let uri = build_url(&host);
    let resp: Option<Value> = client.get(&uri).send().await?.json().await?;

    match resp {
        Some(d) => {
            let subdomains = AnubisResult::new(d).subdomains();

            if !subdomains.is_empty() {
                info!("Discovered {} results for: {}", &subdomains.len(), &host);
                let _ = sender.send(subdomains).await?;
                Ok(())
            } else {
                warn!("No results for: {}", &host);
                Err(Error::source_error("AnubisDB", host))
            }
        }

        None => {
            warn!("No results for: {}", &host);
            Err(Error::source_error("AnubisDB", host))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use std::time::Duration;
    use tokio::sync::mpsc::channel;

    #[test]
    fn url_builder() {
        let correct_uri = "https://jldc.me/anubis/subdomains/hackerone.com";
        assert_eq!(correct_uri, build_url("hackerone.com"));
    }

    // Checks to see if the run function returns subdomains
    #[tokio::test]
    async fn returns_results() {
        let (tx, mut rx) = channel(1);
        let host = Arc::new("hackerone.com".to_string());
        let client = client!(25, 25);
        let _ = run(client, host, tx).await.unwrap();
        let mut results = Vec::new();
        for r in rx.recv().await {
            results.extend(r)
        }
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn handle_no_results() {
        let (tx, _rx) = channel(1);
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        let client = client!(25, 25);
        let res = run(client, host, tx).await;
        let e = res.unwrap_err();
        assert_eq!(
            e.to_string(),
            "AnubisDB couldn't find any results for: anVubmxpa2VzdGVh.com"
        );
    }
}
