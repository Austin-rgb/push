use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::{from_str, to_string};

use crate::ws::MessageOnTrans;

pub struct Deliverer {
    pub client: RequestBuilder,
}

#[derive(Deserialize)]
pub struct Report {
    delivered: bool,
}

impl Deliverer {
    pub async fn deliver(&self, message: MessageOnTrans) -> bool {
        if let Ok(response) = self
            .client
            .try_clone()
            .unwrap()
            .body(to_string(&message).unwrap())
            .send()
            .await
        {
            let text = match response.text().await {
                Ok(t) => t,
                Err(_) => return false,
            };
            let report: Report = match from_str::<Report>(&text) {
                Ok(r) => r,
                Err(_) => return false,
            };
            report.delivered
        } else {
            false
        }
    }

    pub async fn new(url: String, access: String) -> Self {
        let client = Client::new().post(url).bearer_auth(access);
        Self { client }
    }
}
