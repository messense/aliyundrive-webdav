use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use ::time::{Format, OffsetDateTime};
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures::FutureExt;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{oneshot, RwLock},
    time,
};
use webdav_handler::fs::{DavDirEntry, DavMetaData, FsError, FsFuture, FsResult};

const API_BASE_URL: &str = "https://api.aliyundrive.com";
pub const ORIGIN: &str = "https://www.aliyundrive.com";
pub const REFERER: &str = "https://www.aliyundrive.com/";
pub const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";

#[derive(Debug, Clone)]
struct Credentials {
    refresh_token: String,
    access_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AliyunDrive {
    client: reqwest::Client,
    credentials: Arc<RwLock<Credentials>>,
    pub drive_id: Option<String>,
}

impl AliyunDrive {
    pub async fn new(refresh_token: String) -> Result<Self> {
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
                        error!("send default drive id failed");
                    }
                }
                Err(err) => {
                    error!("refresh token failed: {}", err);
                    tx.send(String::new()).unwrap();
                }
            }
            loop {
                time::sleep(time::Duration::from_secs(delay_seconds)).await;
                if let Err(err) = client.do_refresh_token().await {
                    error!("refresh token failed: {}", err);
                }
            }
        });

        let drive_id = rx.await?;
        if drive_id.is_empty() {
            bail!("get default drive id failed");
        }
        info!("default drive id is {}", drive_id);
        drive.drive_id = Some(drive_id);

        Ok(drive)
    }

    async fn do_refresh_token(&self) -> Result<RefreshTokenResponse> {
        let mut cred = self.credentials.write().await;
        let mut data = HashMap::new();
        data.insert("refresh_token", &cred.refresh_token);
        let res = self
            .client
            .post("https://websv.aliyundrive.com/token/refresh")
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
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

    async fn access_token(&self) -> Result<String> {
        let cred = self.credentials.read().await;
        Ok(cred.access_token.clone().context("missing access_token")?)
    }

    fn drive_id(&self) -> Result<&str> {
        self.drive_id.as_deref().context("missing drive_id")
    }

    pub async fn list_all(&self, parent_file_id: &str) -> Result<Vec<AliyunFile>> {
        let mut files = Vec::new();
        let mut marker = None;
        loop {
            let res = self.list(parent_file_id, marker.as_deref()).await?;
            files.extend(res.items.into_iter());
            if res.next_marker.is_empty() {
                break;
            }
            marker = Some(res.next_marker);
        }
        Ok(files)
    }

    pub async fn list(
        &self,
        parent_file_id: &str,
        marker: Option<&str>,
    ) -> Result<ListFileResponse> {
        let req = ListFileRequest {
            drive_id: self.drive_id()?,
            parent_file_id,
            limit: 100,
            all: false,
            image_thumbnail_process: "image/resize,w_400/format,jpeg",
            image_url_process: "image/resize,w_1920/format,jpeg",
            video_thumbnail_process: "video/snapshot,t_0,f_jpg,ar_auto,w_300",
            fields: "*",
            order_by: "updated_at",
            order_direction: "DESC",
            marker,
        };

        let access_token = self.access_token().await?;

        let res = self
            .client
            .post(format!("{}/adrive/v3/file/list", API_BASE_URL))
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("User-Agent", UA)
            .bearer_auth(&access_token)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        let res = res.json::<ListFileResponse>().await?;
        Ok(res)
    }

    pub async fn get(&self, file_id: &str) -> Result<AliyunFile> {
        let req = GetFileRequest {
            drive_id: self.drive_id()?,
            file_id,
            image_thumbnail_process: "image/resize,w_400/format,jpeg",
            image_url_process: "image/resize,w_1920/format,jpeg",
            video_thumbnail_process: "video/snapshot,t_0,f_jpg,ar_auto,w_300",
            fields: "*",
        };

        let access_token = self.access_token().await?;

        let res = self
            .client
            .post(format!("{}/v2/file/get", API_BASE_URL))
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("User-Agent", UA)
            .bearer_auth(&access_token)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        let res = res.json::<AliyunFile>().await?;
        Ok(res)
    }

    pub async fn download(&self, url: &str, start_pos: u64, size: usize) -> Result<Bytes> {
        use reqwest::header::RANGE;

        let end_pos = start_pos + size as u64 - 1;
        trace!("download {} from {} to {}", url, start_pos, end_pos);
        let range = format!("bytes={}-{}", start_pos, end_pos);
        let res = self
            .client
            .get(url)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("User-Agent", UA)
            .header(RANGE, range)
            .send()
            .await?
            .error_for_status()?;
        Ok(res.bytes().await?)
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

#[derive(Debug, Clone, Serialize)]
struct ListFileRequest<'a> {
    drive_id: &'a str,
    parent_file_id: &'a str,
    limit: u64,
    all: bool,
    image_thumbnail_process: &'a str,
    image_url_process: &'a str,
    video_thumbnail_process: &'a str,
    fields: &'a str,
    order_by: &'a str,
    order_direction: &'a str,
    marker: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListFileResponse {
    pub items: Vec<AliyunFile>,
    pub next_marker: String,
}

#[derive(Debug, Clone, Serialize)]
struct GetFileRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    image_thumbnail_process: &'a str,
    image_url_process: &'a str,
    video_thumbnail_process: &'a str,
    fields: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AliyunFile {
    pub drive_id: String,
    pub name: String,
    #[serde(rename = "file_id")]
    pub id: String,
    pub r#type: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub size: u64,
    pub download_url: Option<String>,
}

impl DavMetaData for AliyunFile {
    fn len(&self) -> u64 {
        self.size
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(OffsetDateTime::parse(&self.updated_at, Format::Rfc3339)
            .map_err(|_| FsError::GeneralFailure)?
            .into())
    }

    fn is_dir(&self) -> bool {
        self.r#type == "folder"
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(OffsetDateTime::parse(&self.created_at, Format::Rfc3339)
            .map_err(|_| FsError::GeneralFailure)?
            .into())
    }
}

impl DavDirEntry for AliyunFile {
    fn name(&self) -> Vec<u8> {
        self.name.as_bytes().to_vec()
    }

    fn metadata(&self) -> FsFuture<Box<dyn DavMetaData>> {
        async move { Ok(Box::new(self.clone()) as Box<dyn DavMetaData>) }.boxed()
    }
}
