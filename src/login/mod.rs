pub mod model;

use crate::drive::DriveConfig;
use crate::login::model::*;

pub struct QrCodeScanner {
    client: reqwest::Client,
    drive_config: DriveConfig,
}

impl QrCodeScanner {
    pub async fn new(drive_config: DriveConfig) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(50))
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            drive_config,
        })
    }
}

impl QrCodeScanner {
    pub async fn scan(&self) -> anyhow::Result<QrCodeResponse> {
        let req = QrCodeRequest {
            client_id: self.drive_config.client_id.clone(),
            client_secret: self.drive_config.client_secret.clone(),
            scopes: vec![
                "user:base".to_string(),
                "file:all:read".to_string(),
                "file:all:write".to_string(),
            ],
            width: None,
            height: None,
        };
        let url =
            if self.drive_config.client_id.is_none() || self.drive_config.client_secret.is_none() {
                format!(
                    "{}/oauth/authorize/qrcode",
                    &self.drive_config.refresh_token_host
                )
            } else {
                "https://openapi.aliyundrive.com/oauth/authorize/qrcode".to_string()
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
            client_id: self.drive_config.client_id.clone(),
            client_secret: self.drive_config.client_secret.clone(),
            grant_type: "authorization_code".to_string(),
            code: code.to_string(),
        };
        let url =
            if self.drive_config.client_id.is_none() || self.drive_config.client_secret.is_none() {
                format!(
                    "{}/oauth/access_token",
                    &self.drive_config.refresh_token_host
                )
            } else {
                "https://openapi.aliyundrive.com/oauth/access_token".to_string()
            };
        let resp = self.client.post(url).json(&req).send().await?;
        let resp = resp.json::<AuthorizationCodeResponse>().await?;
        Ok(resp.refresh_token)
    }
}
