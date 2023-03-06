pub mod model;

use crate::login::model::*;

pub struct QrCodeScanner {
    client: reqwest::Client,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl QrCodeScanner {
    pub async fn new(
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(50))
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            client_id,
            client_secret,
        })
    }
}

impl QrCodeScanner {
    pub async fn scan(&self) -> anyhow::Result<QrCodeResponse> {
        let req = QrCodeRequest {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            scopes: vec![
                "user:base".to_string(),
                "file:all:read".to_string(),
                "file:all:write".to_string(),
            ],
            width: None,
            height: None,
        };
        let url = if self.client_id.is_none() || self.client_secret.is_none() {
            "https://aliyundrive-oauth.messense.me/oauth/authorize/qrcode"
        } else {
            "https://openapi.aliyundrive.com/oauth/authorize/qrcode"
        };
        let resp = self.client.post(url).json(&req).send().await?;
        let resp = resp.json::<QrCodeResponse>().await?;
        Ok(resp)
    }

    pub async fn query(&self, sid: &str) -> anyhow::Result<QrCodeStatusResponse> {
        let url = format!("https://openapi.aliyundrive.com/oauth/qrcode/{sid}/status");
        let resp = self.client.get(url).send().await?;
        let resp = resp.json::<QrCodeStatusResponse>().await?;
        Ok(resp)
    }

    pub async fn fetch_refresh_token(&self, code: &str) -> anyhow::Result<String> {
        let req = AuthorizationCodeRequest {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            grant_type: "authorization_code".to_string(),
            code: code.to_string(),
        };
        let url = if self.client_id.is_none() || self.client_secret.is_none() {
            "https://aliyundrive-oauth.messense.me/oauth/access_token"
        } else {
            "https://openapi.aliyundrive.com/oauth/access_token"
        };
        let resp = self.client.post(url).json(&req).send().await?;
        let resp = resp.json::<AuthorizationCodeResponse>().await?;
        Ok(resp.refresh_token)
    }
}
