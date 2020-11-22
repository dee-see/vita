use crate::error::{Error, Result};
use crate::{DataSource, IntoSubdomain};
use async_trait::async_trait;
use dotenv::dotenv;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

#[derive(Debug, PartialEq)]
struct Creds {
    app_id: String,
    app_secret: String,
}

impl Creds {
    pub fn read_creds() -> Result<Self> {
        dotenv().ok();
        let app_id = env::var("FB_APP_ID");
        let app_secret = env::var("FB_APP_SECRET");

        if app_id.is_ok() && app_secret.is_ok() {
            Ok(Self {
                app_id: app_id?,
                app_secret: app_secret?,
            })
        } else {
            Err(Error::key_error(
                "Facebook",
                &["FB_APP_ID", "FB_APP_SECRET"],
            ))
        }
    }

    pub async fn authenticate(&self, client: Client) -> Result<String> {
        // created a struct because deserializing into a serde_json::Value
        // was returning the access token with quotation marks"tokeninhere"
        // but wasn't doing that as a struct.
        #[derive(Deserialize)]
        struct AuthResp {
            access_token: String,
        }

        let auth_url = format!(
            "https://graph.facebook.com/oauth/access_token?client_id={}\
            &client_secret={}&grant_type=client_credentials",
            self.app_id, self.app_secret
        );

        let resp: Option<AuthResp> = client.get(&auth_url).send().await?.json().await?;

        if let Some(r) = resp {
            Ok(r.access_token)
        } else {
            Err(Error::auth_error("Facebook"))
        }
    }
}

#[derive(Deserialize, Debug)]
struct Subdomains {
    domains: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct FacebookResult {
    data: Vec<Subdomains>,
}

impl IntoSubdomain for FacebookResult {
    fn subdomains(&self) -> Vec<String> {
        self.data
            .iter()
            .flat_map(|s| s.domains.to_owned())
            .collect()
    }
}

//TODO: creds should probably be provided on Facebook::new
#[derive(Default, Clone)]
pub struct Facebook {
    client: Client,
}

impl Facebook {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn build_url(&self, host: &str, token: &str) -> String {
        format!(
            "https://graph.facebook.com/certificates?fields=domains&access_token={}&query=*.{}",
            token, host
        )
    }
}

#[async_trait]
impl DataSource for Facebook {
    async fn run(&self, host: Arc<String>, mut tx: Sender<Vec<String>>) -> Result<()> {
        let access_token = match Creds::read_creds() {
            Ok(c) => c.authenticate(self.client.clone()).await?,
            Err(_) => {
                warn!("Couldn't authenticate to Facebook, ignoring");
                return Err(Error::key_error(
                    "Facebook",
                    &["FB_APP_ID", "FB_APP_SECRET"],
                ));
            }
        };

        let uri = self.build_url(&host, &access_token);
        let resp: Option<FacebookResult> = self.client.get(&uri).send().await?.json().await?;

        match resp {
            Some(data) => {
                let subdomains = data.subdomains();
                if !subdomains.is_empty() {
                    info!("Discovered {} results for {}", &subdomains.len(), &host);
                    let _ = tx.send(subdomains).await?;
                    Ok(())
                } else {
                    Err(Error::source_error("Facebook", host))
                }
            }
            None => {
                warn!("No results for: {}", &host);
                Err(Error::source_error("Facebook", host))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use std::time::Duration;
    use tokio::sync::mpsc::channel;

    // checks if we can fetch the credentials from an .env file.
    #[ignore]
    #[test]
    fn get_creds() {
        dotenv().ok();
        let app_id = env::var("FB_APP_ID").unwrap();
        let app_secret = env::var("FB_APP_SECRET").unwrap();
        let creds: Creds = Creds { app_id, app_secret };
        assert_eq!(creds, Creds::read_creds().unwrap());
    }

    // ignoring passed locally
    #[ignore]
    #[test]
    fn get_no_creds() {
        let creds = Creds::read_creds();
        let e = creds.unwrap_err();
        let correct_msg = r#"Couldn't read ["FB_APP_ID", "FB_APP_SECRET"] for Facebook. Check if you have them set."#;
        assert_eq!(e.to_string(), correct_msg);
    }

    // Checks if we can authenticate with Facebook.
    #[ignore]
    #[tokio::test]
    async fn auth() {
        let client = client!();
        let token = Creds::read_creds()
            .unwrap()
            .authenticate(client)
            .await
            .unwrap();
        assert!(token.len() > 1);
    }

    // Checks to see if the run function returns subdomains
    #[ignore]
    #[tokio::test]
    async fn returns_results() {
        let (tx, mut rx) = channel(1);
        let host = Arc::new("hackerone.com".to_owned());
        let _ = Facebook::default().run(host, tx).await;
        let mut results = Vec::new();
        for r in rx.recv().await {
            results.extend(r)
        }
        assert!(!results.is_empty());
    }

    // Checks that if we get no results that we just return an error.
    // test is ignored by default to preserve limits
    #[ignore]
    #[tokio::test]
    async fn handle_no_results() {
        let (tx, _rx) = channel(1);
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        let res = Facebook::default().run(host, tx).await;
        let e = res.unwrap_err();
        assert_eq!(
            e.to_string(),
            "Facebook couldn't find any results for: anVubmxpa2VzdGVh.com"
        );
    }
}
