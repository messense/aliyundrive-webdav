use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct QrCodeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QrCodeResponse {
    #[serde(rename = "qrCodeUrl")]
    pub qr_code_url: String,
    pub sid: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum QrCodeStatus {
    WaitLogin,
    ScanSuccess,
    LoginSuccess,
    QrCodeExpired,
}

impl FromStr for QrCodeStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use QrCodeStatus::*;

        match s {
            "WaitLogin" => Ok(WaitLogin),
            "ScanSuccess" => Ok(ScanSuccess),
            "LoginSuccess" => Ok(LoginSuccess),
            _ => Ok(QrCodeExpired),
        }
    }
}

impl<'de> Deserialize<'de> for QrCodeStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QrCodeStatusResponse {
    pub status: QrCodeStatus,
    #[serde(rename = "authCode")]
    pub auth_code: Option<String>,
}

impl QrCodeStatusResponse {
    pub fn is_success(&self) -> bool {
        matches!(self.status, QrCodeStatus::LoginSuccess)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationCodeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub grant_type: String,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizationCodeResponse {
    // pub token_type: String,
    // pub access_token: String,
    pub refresh_token: String,
    // pub expires_in: usize,
}
