use bytes::Bytes;

pub mod model;

use crate::login::model::*;

const QRCODE_API: &str = "https://openapi.aliyundrive.com/oauth/authorize/qrcode";

pub struct QrCodeScanner {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
}

impl QrCodeScanner {
    pub async fn new(client_id: String, client_secret: String) -> anyhow::Result<Self> {
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
        let resp = self.client.post(QRCODE_API).json(&req).send().await?;
        let resp = resp.json::<QrCodeResponse>().await?;
        Ok(resp)
    }

    pub async fn qrcode(&self, sid: &str) -> anyhow::Result<Bytes> {
        let url = format!("https://openapi.aliyundrive.com/oauth/qrcode/{sid}");
        let resp = self.client.get(url).send().await?;
        let content = resp.bytes().await?;
        Ok(content)
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
        let resp = self
            .client
            .post("https://openapi.aliyundrive.com/oauth/access_token")
            .json(&req)
            .send()
            .await?;
        let resp = resp.json::<AuthorizationCodeResponse>().await?;
        Ok(resp.refresh_token)
    }
}
