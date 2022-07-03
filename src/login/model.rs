use anyhow::{bail, Context};

use crate::login::State;
use serde::{Deserialize, Serialize};
use url::Url;
pub const CODE_KEY: &str = "code";
pub const LOGIN_TYPE: &str = "normal";
pub const CK_KEY: &str = "ck";
pub const T_KEY: &str = "t";

pub trait CkForm {
    fn map_form(&self) -> std::collections::HashMap<String, String>;
}

pub trait AuthorizationToken {
    fn access_token(&self) -> Option<String>;

    fn refresh_token(&self) -> Option<String>;
}

pub trait Ok {
    fn ok(&self) -> bool;
}

// build qrcode result
#[derive(Deserialize)]
pub struct GeneratorQrCodeResult {
    #[serde(default)]
    #[serde(rename = "content")]
    content: Option<GeneratorQrCodeContent>,

    #[serde(rename = "hasError")]
    #[serde(default)]
    has_error: bool,
}

impl GeneratorQrCodeResult {
    pub fn get_content(&self) -> String {
        if let Some(ref content) = self.content {
            if let Some(ref data) = content.data {
                let code_content = match &data.code_content {
                    None => String::new(),
                    Some(code_content) => code_content.to_string(),
                };
                return code_content;
            }
        }
        String::new()
    }

    pub fn get_content_data(self) -> Option<GeneratorQrCodeContentData> {
        self.content?.data
    }
}

impl Ok for GeneratorQrCodeResult {
    fn ok(&self) -> bool {
        if let Some(ref t) = self.content {
            return !self.has_error && t.success;
        }
        !self.has_error
    }
}

#[derive(Deserialize)]
pub struct GeneratorQrCodeContent {
    #[serde(default)]
    #[serde(rename = "data")]
    data: Option<GeneratorQrCodeContentData>,

    #[serde(rename = "success")]
    #[serde(default)]
    success: bool,
}

impl GeneratorQrCodeContent {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            data: None,
            success: false,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct GeneratorQrCodeContentData {
    #[serde(rename = "t")]
    #[serde(default)]
    t: i64,

    #[serde(default)]
    #[serde(rename = "codeContent")]
    code_content: Option<String>,

    #[serde(default)]
    #[serde(rename = "ck")]
    ck: Option<String>,
}

// query qrcode scan status
#[derive(Deserialize)]
pub struct QueryQrCodeResult {
    #[serde(default)]
    #[serde(rename = "content")]
    content: Option<QueryQrCodeContent>,

    #[serde(default)]
    #[serde(rename = "hasError")]
    has_error: bool,
}

impl QueryQrCodeResult {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            content: None,
            has_error: false,
        }
    }

    pub fn get_mobile_login_result(&self) -> Option<MobileLoginResult> {
        let biz_ext = self.get_biz_ext()?;
        let vec = base64::decode(biz_ext).unwrap();
        let string = vec.iter().map(|&c| c as char).collect::<String>();
        serde_json::from_str::<MobileLoginResult>(string.as_str()).ok()
    }

    fn get_biz_ext(&self) -> Option<String> {
        let content = self.content.as_ref()?;
        let data = content.data.as_ref()?;
        let biz_ext = data.biz_ext.as_ref()?;
        Some(biz_ext.to_string())
    }

    fn get_status(&self) -> Option<State> {
        let content = self.content.as_ref()?;
        let data = content.data.as_ref()?;
        let state = data.qr_code_status.as_ref().cloned()?;
        Some(state)
    }
}

impl Ok for QueryQrCodeResult {
    fn ok(&self) -> bool {
        if let Some(ref t) = self.content {
            return !self.has_error && t.success;
        }
        !self.has_error
    }
}

impl QueryQrCodeResult {
    pub fn is_new(&self) -> bool {
        if let Some(ref state) = self.get_status() {
            if State::New.eq(state) {
                return true;
            }
        }
        false
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ref state) = self.get_status() {
            if State::Expired.eq(state) {
                return true;
            }
        }
        false
    }

    pub fn is_confirmed(&self) -> bool {
        if let Some(ref state) = self.get_status() {
            if State::Confirmed.eq(state) {
                return true;
            }
        }
        false
    }
}

#[derive(Deserialize)]
pub struct QueryQrCodeContent {
    #[serde(rename = "data")]
    data: Option<QueryQrCodeContentData>,

    #[serde(default)]
    success: bool,
}

#[derive(Deserialize, PartialEq)]
pub struct QueryQrCodeContentData {
    #[serde(default)]
    #[serde(rename = "qrCodeStatus")]
    qr_code_status: Option<State>,

    #[serde(default)]
    #[serde(rename = "bizExt")]
    biz_ext: Option<String>,
}

// query qrcode status form
#[derive(Serialize, Default)]
pub struct QueryQrCodeCkForm {
    t: i64,
    ck: String,
}

impl QueryQrCodeCkForm {
    pub fn new(t: i64, ck: String) -> Self {
        Self { t, ck }
    }
}

impl From<GeneratorQrCodeResult> for QueryQrCodeCkForm {
    fn from(from: GeneratorQrCodeResult) -> Self {
        if let Some(ref content) = from.content {
            if let Some(ref data) = content.data {
                let ck = match &data.ck {
                    None => String::new(),
                    Some(ck) => ck.to_string(),
                };
                return Self { t: data.t, ck };
            }
        }
        Self {
            t: 0,
            ck: String::new(),
        }
    }
}

impl CkForm for QueryQrCodeCkForm {
    fn map_form(&self) -> std::collections::HashMap<String, String> {
        let mut ck_map = std::collections::HashMap::<String, String>::new();
        ck_map.insert(T_KEY.to_string(), self.t.to_string());
        ck_map.insert(CK_KEY.to_string(), self.ck.to_string());
        ck_map
    }
}

#[derive(Serialize, Debug)]
pub struct Token {
    #[serde(rename = "token")]
    #[serde(default)]
    value: Option<String>,
}

impl From<&String> for Token {
    fn from(token: &String) -> Self {
        Self {
            value: Some(token.to_string()),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct AuthorizationCode {
    #[serde(rename = "code")]
    #[serde(default)]
    code: Option<String>,

    #[serde(rename = "loginType")]
    #[serde(default)]
    login_type: Option<String>,
}

impl From<&GotoResult> for AuthorizationCode {
    fn from(from: &GotoResult) -> Self {
        let code = from.extract_authorization_code();
        match code {
            Ok(code) => {
                return Self {
                    code: Some(code),
                    login_type: Some(LOGIN_TYPE.to_string()),
                };
            }
            Err(e) => {
                eprintln!("get authorization error: {}", e)
            }
        }
        Self {
            code: None,
            login_type: None,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct GotoResult {
    #[serde(default)]
    goto: Option<String>,
}

impl From<&String> for GotoResult {
    fn from(token: &String) -> Self {
        Self {
            goto: Some(token.to_string()),
        }
    }
}

impl GotoResult {
    pub fn extract_authorization_code(&self) -> anyhow::Result<String> {
        let goto = self.goto.as_ref().context("goto value is None")?;
        let url = Url::parse(goto.as_str())?;
        let query = url.query().context("goto query is None")?;
        let param_array = query.split('&').collect::<Vec<&str>>();
        for param in param_array {
            let param = param.to_string();
            let k_v_array = param.split('=').collect::<Vec<&str>>();
            let key = k_v_array.get(0).context("goto query param key is None")?;
            if *key == CODE_KEY {
                let value = k_v_array.get(1).context("goto query param value is None")?;
                return Ok(String::from(*value));
            }
        }
        bail!("Failed to get authorization code")
    }
}

#[derive(Deserialize, Debug)]
pub struct MobileLoginResult {
    #[serde(default)]
    pds_login_result: Option<PdsLoginResult>,
}

impl AuthorizationToken for MobileLoginResult {
    fn access_token(&self) -> Option<String> {
        let pds_login_result = self.pds_login_result.as_ref()?;
        let access_token = pds_login_result.access_token.as_ref()?;
        Some(access_token.to_string())
    }

    fn refresh_token(&self) -> Option<String> {
        let pds_login_result = self.pds_login_result.as_ref()?;
        let refresh_token = pds_login_result.refresh_token.as_ref()?;
        Some(refresh_token.to_string())
    }
}

#[derive(Deserialize, Debug)]
pub struct PdsLoginResult {
    #[serde(rename = "accessToken")]
    #[serde(default)]
    access_token: Option<String>,

    #[serde(rename = "refreshToken")]
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct WebLoginResult {
    #[serde(default)]
    access_token: Option<String>,

    #[serde(default)]
    refresh_token: Option<String>,
}

impl AuthorizationToken for WebLoginResult {
    fn access_token(&self) -> Option<String> {
        self.access_token.as_ref().cloned()
    }

    fn refresh_token(&self) -> Option<String> {
        self.refresh_token.as_ref().cloned()
    }
}
