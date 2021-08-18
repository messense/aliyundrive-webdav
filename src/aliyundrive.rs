use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use log::{error, info};
use serde::Deserialize;
use tokio::{
    sync::{oneshot, RwLock},
    time,
};

const API_BASE_URL: &str = "https://api.aliyundrive.com/v2/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";

#[derive(Debug, Clone)]
struct Credentials {
    refresh_token: String,
    access_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AliyunDrive {
    client: reqwest::Client,
    credentials: Arc<RwLock<Credentials>>,
    drive_id: Option<String>,
}

impl AliyunDrive {
    pub async fn new(refresh_token: String) -> Self {
        let credentials = Credentials {
            refresh_token,
            access_token: None,
        };
        let mut drive = Self {
            client: reqwest::Client::new(),
            credentials: Arc::new(RwLock::new(credentials)),
            drive_id: None,
        };

        let (tx, rx) = oneshot::channel();
        // schedule update token task
        let client = drive.clone();
        tokio::spawn(async move {
            let mut delay_seconds = 7000;
            match client.do_refresh_token().await {
                Ok(res) => {
                    // token usually expires in 7200s, refresh earlier
                    delay_seconds = res.expires_in - 200;
                    if tx.send(res.default_drive_id).is_err() {
                        error!("the receiver dropped");
                    }
                }
                Err(err) => error!("refresh token failed: {}", err),
            }
            loop {
                time::sleep(time::Duration::from_secs(delay_seconds)).await;
                if let Err(err) = client.do_refresh_token().await {
                    error!("refresh token failed: {}", err);
                }
            }
        });

        match rx.await {
            Ok(drive_id) => {
                info!("default drive id is {}", drive_id);
                drive.drive_id = Some(drive_id);
            }
            Err(_) => error!("the sender dropped"),
        }

        drive
    }

    async fn do_refresh_token(&self) -> Result<RefreshTokenResponse> {
        let mut cred = self.credentials.write().await;
        let mut data = HashMap::new();
        data.insert("refresh_token", &cred.refresh_token);
        let res = self
            .client
            .post("https://websv.aliyundrive.com/token/refresh")
            .header("Content-Type", "application/json")
            .header("Origin", "https://www.aliyundrive.com")
            .header("Referer", "https://www.aliyundrive.com")
            .header("User-Agent", UA)
            .json(&data)
            .send()
            .await?
            .error_for_status()?;
        let res = res.json::<RefreshTokenResponse>().await?;
        cred.refresh_token = res.refresh_token.clone();
        cred.access_token = Some(res.access_token.clone());
        info!("refresh token succeed for {}", res.nick_name);
        Ok(res)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
    user_id: String,
    nick_name: String,
    default_drive_id: String,
}
