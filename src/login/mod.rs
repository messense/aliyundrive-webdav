pub mod model;

use crate::login::model::{CkForm, GeneratorQrCodeResult, QueryQrCodeResult};
use anyhow::anyhow;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;

// generator qrcode
const GENERATOR_QRCODE_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/generate.do?appName=aliyun_drive&fromSite=52&appEntrance=web";
// query scanner result (include mobile token)
const QUERY_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/query.do?appName=aliyun_drive&fromSite=52&_bx-v=2.0.31";

#[derive(Eq, PartialEq, Clone)]
pub enum State {
    Confirmed,
    Expired,
    New,
}

impl FromStr for State {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use State::*;

        match s {
            "NEW" => Ok(New),
            "EXPIRED" => Ok(Expired),
            "CONFIRMED" => Ok(Confirmed),
            _ => Ok(Expired),
        }
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

pub struct QrCodeScanner {
    client: reqwest::Client,
}

impl QrCodeScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(50))
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { client })
    }
}

impl QrCodeScanner {
    pub async fn generator(&self) -> anyhow::Result<GeneratorQrCodeResult> {
        let resp = self.client.get(GENERATOR_QRCODE_API).send().await?;
        ResponseHandler::response_handler::<GeneratorQrCodeResult>(resp).await
    }

    pub async fn query(&self, from: &impl CkForm) -> anyhow::Result<QueryQrCodeResult> {
        let resp = self
            .client
            .post(QUERY_API)
            .form(&from.map_form())
            .send()
            .await?;
        ResponseHandler::response_handler::<QueryQrCodeResult>(resp).await
    }
}

struct ResponseHandler;

impl ResponseHandler {
    async fn response_handler<T: DeserializeOwned>(resp: Response) -> anyhow::Result<T> {
        if resp.status().is_success() {
            let result = resp.json::<T>().await?;
            return Ok(result);
        }
        let msg = ResponseHandler::response_error_msg_handler(resp).await;
        Err(anyhow!(msg))
    }

    async fn response_error_msg_handler(resp: Response) -> String {
        resp.text()
            .await
            .unwrap_or_else(|e| format!("An error occurred while extracting the body: {:?}", e))
    }
}
