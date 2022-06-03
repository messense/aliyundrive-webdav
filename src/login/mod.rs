pub mod model;

use crate::login::model::{
    AuthorizationCode, CkForm, GeneratorQrCodeResult, GotoResult, QueryQrCodeResult, Token,
    WebLoginResult,
};
use anyhow::anyhow;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;

// generator qrcode
const GENERATOR_QRCODE_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/generate.do?appName=aliyun_drive&fromSite=52&appEntrance=web";
// query scanner result (include mobile token)
const QUERY_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/query.do?appName=aliyun_drive&fromSite=52&_bx-v=2.0.31";
// get session id
const SESSION_ID_API: &str = "https://auth.aliyundrive.com/v2/oauth/authorize?client_id=25dzX3vbYqktVxyX&redirect_uri=https%3A%2F%2Fwww.aliyundrive.com%2Fsign%2Fcallback&response_type=code&login_type=custom&state=%7B%22origin%22%3A%22https%3A%2F%2Fwww.aliyundrive.com%22%7D#/nestedlogin?keepLogin=false&hidePhoneCode=true&ad__pass__q__rememberLogin=true&ad__pass__q__rememberLoginDefaultValue=true&ad__pass__q__forgotPassword=true&ad__pass__q__licenseMargin=true&ad__pass__q__loginType=normal";
// scan scan result（include authorization code）
const TOKEN_LOGIN_API: &str = "https://auth.aliyundrive.com/v2/oauth/token_login";
// get web side token
const GET_WEB_TOKEN_API: &str = "https://api.aliyundrive.com/token/get";

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36";
const SESSION_ID_KEY: &str = "SESSIONID";

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
    session_id: String,
    client: reqwest::Client,
}

impl QrCodeScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(50))
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client
            .get(SESSION_ID_API)
            .header(reqwest::header::USER_AGENT, UA)
            .send()
            .await?;
        if resp.status().is_success() {
            for cookie in resp.cookies() {
                if cookie.name() == SESSION_ID_KEY {
                    return Ok(Self {
                        session_id: String::from(cookie.value()),
                        client,
                    });
                }
            }
        }
        return Err(anyhow!("Failed to get session id."));
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

    pub async fn token_login(&self, token: Token) -> anyhow::Result<GotoResult> {
        let resp = self
            .client
            .post(TOKEN_LOGIN_API)
            .header(
                reqwest::header::COOKIE,
                format!("SESSIONID={}", &self.session_id),
            )
            .json(&token)
            .send()
            .await?;
        ResponseHandler::response_handler::<GotoResult>(resp).await
    }

    pub async fn get_token(&self, auth: AuthorizationCode) -> anyhow::Result<WebLoginResult> {
        let resp = self
            .client
            .post(GET_WEB_TOKEN_API)
            .header(
                reqwest::header::COOKIE,
                format!("SESSIONID={}", &self.session_id),
            )
            .json(&auth)
            .send()
            .await?;
        ResponseHandler::response_handler::<WebLoginResult>(resp).await
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
