pub mod model;

use crate::login::model::{
    AuthorizationCode, CkForm, GeneratorQrCodeResult, GotoResult, QueryQrCodeResult, Token,
    WebLoginResult,
};
use crate::login::State::{CONFIRMED, EXPIRED, NEW};
use anyhow::anyhow;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;

// generator qrcode
const GENERATOR_QRCODE_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/generate.do?appName=aliyun_drive&fromSite=52&appEntrance=web&lang=zh_CN";
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
    CONFIRMED,
    EXPIRED,
    NEW,
}

impl FromStr for State {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NEW" => Ok(NEW),
            "EXPIRED" => Ok(EXPIRED),
            "CONFIRMED" => Ok(CONFIRMED),
            _ => Ok(EXPIRED),
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
            .unwrap_or(String::from("An error occurred while extracting the body."))
    }
}

#[cfg(test)]
mod tests {

    use crate::login;
    use crate::login::model::{
        AuthorizationCode, AuthorizationToken, Ok, QueryQrCodeCkForm, Token,
    };

    #[tokio::test]
    async fn test() {
        let scan = login::QrCodeScanner::new().await.unwrap();
        // 返回二维码内容结果集
        let generator_qr_code_result = scan.generator().await.unwrap();
        // 需要生成二维码的内容
        let qrcode_content = generator_qr_code_result.get_content();
        let ck_form = QueryQrCodeCkForm::from(generator_qr_code_result);
        // 打印二维码
        qr2term::print_qr(qrcode_content).unwrap();
        for i in 0..10 {
            // 模拟轮训查询二维码状态
            let query_result = scan.query(&ck_form).await.unwrap();
            if query_result.ok() {
                // 扫码成功
                if query_result.is_confirmed() {
                    // 获取移动端登陆Result
                    let mobile_login_result = query_result.get_mobile_login_result().unwrap();
                    // 移动端access-token
                    let access_token = mobile_login_result.access_token().unwrap_or(String::new());
                    // 根据移动端access-token获取authorization code（授权码）
                    let goto_result = scan.token_login(Token::from(&access_token)).await.unwrap();
                    // 根据授权码登陆获取Web端登陆结果集
                    let web_login_result = scan
                        .get_token(AuthorizationCode::from(&goto_result))
                        .await
                        .unwrap();
                    // 获取Web端refresh token
                    let refresh_token = web_login_result.refresh_token().unwrap();
                    println!("refresh-token: {:?}", refresh_token);
                    break;
                }
                if query_result.is_expired() {
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }
}
