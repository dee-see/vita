use crate::Result;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
struct CertSpotterResult {
    dns_names: Vec<String>,
}

fn build_url(host: &str) -> String {
    format!(
        "https://api.certspotter.com/v1/issuances?domain={}\
        &include_subdomains=true&expand=dns_names",
        host
    )
}

pub async fn run(host: String) -> Result<HashSet<String>> {
    let uri = build_url(&host);
    let mut results = HashSet::new();
    let resp: Option<Vec<CertSpotterResult>> = surf::get(uri).recv_json().await?;

    match resp {
        Some(data) => data
            .into_iter()
            .flat_map(|s| s.dns_names.into_iter())
            .map(|s| results.insert(s))
            .for_each(drop),
        None => eprintln!("CertSpotter couldn't find results for: {}", &host),
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_await_test::async_test;

    #[test]
    fn url_builder() {
        let correct_uri = "https://api.certspotter.com/v1/issuances?domain=hackerone.com\
        &include_subdomains=true&expand=dns_names";
        assert_eq!(correct_uri, build_url("hackerone.com"));
    }

    // Checks to see if the run function returns subdomains
    #[async_test]
    async fn returns_results() {
        let host = "hackerone.com".to_owned();
        let results = run(host).await.unwrap();
        assert!(results.len() > 3);
    }

    #[async_test]
    async fn handle_no_results() {
        let host = "anVubmxpa2VzdGVh.com".to_owned();
        let results = run(host).await.unwrap();
        assert!(results.len() < 1);
    }
}
