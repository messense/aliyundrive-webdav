use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ::time::{format_description::well_known::Rfc3339, OffsetDateTime};
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures_util::future::FutureExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::{
    sync::{oneshot, RwLock},
    time,
};
use tracing::{debug, error, info, warn};
use webdav_handler::fs::{DavDirEntry, DavMetaData, FsFuture, FsResult};

const API_BASE_URL: &str = "https://api.aliyundrive.com";
const ORIGIN: &str = "https://www.aliyundrive.com";
const REFERER: &str = "https://www.aliyundrive.com/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";
pub const UPLOAD_CHUNK_SIZE: u64 = 16 * 1024 * 1024; // 16MB

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
    workdir: Option<PathBuf>,
}

impl AliyunDrive {
    pub async fn new(refresh_token: String, workdir: Option<PathBuf>) -> Result<Self> {
        let credentials = Credentials {
            refresh_token,
            access_token: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let mut drive = Self {
            client,
            credentials: Arc::new(RwLock::new(credentials)),
            drive_id: None,
            workdir,
        };

        let (tx, rx) = oneshot::channel();
        // schedule update token task
        let client = drive.clone();
        let refresh_token_from_file = if let Some(dir) = drive.workdir.as_ref() {
            tokio::fs::read_to_string(dir.join("refresh_token"))
                .await
                .ok()
        } else {
            None
        };
        tokio::spawn(async move {
            let mut delay_seconds = 7000;
            match client
                .do_refresh_token_with_retry(refresh_token_from_file)
                .await
            {
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
                if let Err(err) = client.do_refresh_token_with_retry(None).await {
                    error!("refresh token failed: {}", err);
                }
            }
        });

        let drive_id = rx.await?;
        if drive_id.is_empty() {
            bail!("get default drive id failed");
        }
        info!(drive_id = %drive_id, "found default drive");
        drive.drive_id = Some(drive_id);

        Ok(drive)
    }

    async fn save_refresh_token(&self, refresh_token: &str) -> Result<()> {
        if let Some(dir) = self.workdir.as_ref() {
            tokio::fs::create_dir_all(dir).await?;
            let refresh_token_file = dir.join("refresh_token");
            tokio::fs::write(refresh_token_file, refresh_token).await?;
        }
        Ok(())
    }

    async fn do_refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenResponse> {
        let mut data = HashMap::new();
        data.insert("refresh_token", refresh_token);
        let res = self
            .client
            .post("https://websv.aliyundrive.com/token/refresh")
            .json(&data)
            .send()
            .await?;
        match res.error_for_status_ref() {
            Ok(_) => {
                let res = res.json::<RefreshTokenResponse>().await?;
                let mut cred = self.credentials.write().await;
                cred.refresh_token = res.refresh_token.clone();
                cred.access_token = Some(res.access_token.clone());
                info!(
                    refresh_token = %res.refresh_token,
                    nick_name = %res.nick_name,
                    "refresh token succeed"
                );
                if let Err(err) = self.save_refresh_token(&res.refresh_token).await {
                    error!(error = %err, "save refresh token failed");
                }
                Ok(res)
            }
            Err(err) => {
                let msg = res.text().await?;
                let context = format!("{}: {}", err, msg);
                Err(err).context(context)
            }
        }
    }

    async fn do_refresh_token_with_retry(
        &self,
        refresh_token_from_file: Option<String>,
    ) -> Result<RefreshTokenResponse> {
        let mut last_err = None;
        let mut refresh_token = self.refresh_token().await;
        for _ in 0..10 {
            match self.do_refresh_token(&refresh_token).await {
                Ok(res) => return Ok(res),
                Err(err) => {
                    let mut should_retry = match err.downcast_ref::<reqwest::Error>() {
                        Some(e) => e.is_connect() || e.is_timeout(),
                        None => false,
                    };
                    // retry if command line refresh_token is invalid but we also have
                    // refresh_token from file
                    if let Some(refresh_token_from_file) = refresh_token_from_file.as_ref() {
                        if !should_retry && &refresh_token != refresh_token_from_file {
                            refresh_token = refresh_token_from_file.trim().to_string();
                            should_retry = true;
                        }
                    }
                    if should_retry {
                        warn!(error = %err, "refresh token failed, will wait and try");
                        last_err = Some(err);
                        time::sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        last_err = Some(err);
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }

    async fn refresh_token(&self) -> String {
        let cred = self.credentials.read().await;
        cred.refresh_token.clone()
    }

    async fn access_token(&self) -> Result<String> {
        let cred = self.credentials.read().await;
        Ok(cred.access_token.clone().context("missing access_token")?)
    }

    fn drive_id(&self) -> Result<&str> {
        self.drive_id.as_deref().context("missing drive_id")
    }

    async fn request<T, U>(&self, url: String, req: &T) -> Result<Option<U>>
    where
        T: Serialize + ?Sized,
        U: DeserializeOwned,
    {
        use reqwest::StatusCode;

        let mut access_token = self.access_token().await?;
        let url = reqwest::Url::parse(&url)?;
        let res = self
            .client
            .post(url.clone())
            .bearer_auth(&access_token)
            .json(&req)
            .send()
            .await?
            .error_for_status();
        match res {
            Ok(res) => {
                if res.status() == StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                let res = res.json::<U>().await?;
                Ok(Some(res))
            }
            Err(err) => {
                match err.status() {
                    Some(
                        status_code
                        @
                        (StatusCode::UNAUTHORIZED
                        | StatusCode::INTERNAL_SERVER_ERROR
                        | StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT),
                    ) => {
                        if status_code == StatusCode::UNAUTHORIZED {
                            // refresh token and retry
                            let token_res = self.do_refresh_token_with_retry(None).await?;
                            access_token = token_res.access_token;
                        } else {
                            // wait for a while and retry
                            time::sleep(Duration::from_secs(1)).await;
                        }
                        let res = self
                            .client
                            .post(url)
                            .bearer_auth(&access_token)
                            .json(&req)
                            .send()
                            .await?
                            .error_for_status()?;
                        if res.status() == StatusCode::NO_CONTENT {
                            return Ok(None);
                        }
                        let res = res.json::<U>().await?;
                        Ok(Some(res))
                    }
                    _ => Err(err.into()),
                }
            }
        }
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
        let drive_id = self.drive_id()?;
        debug!(drive_id = %drive_id, parent_file_id = %parent_file_id, marker = ?marker, "list file");
        let req = ListFileRequest {
            drive_id,
            parent_file_id,
            limit: 200,
            all: false,
            image_thumbnail_process: "image/resize,w_400/format,jpeg",
            image_url_process: "image/resize,w_1920/format,jpeg",
            video_thumbnail_process: "video/snapshot,t_0,f_jpg,ar_auto,w_300",
            fields: "*",
            order_by: "updated_at",
            order_direction: "DESC",
            marker,
        };
        self.request(format!("{}/adrive/v3/file/list", API_BASE_URL), &req)
            .await
            .and_then(|res| res.context("expect response"))
    }

    pub async fn download(
        &self,
        file_id: &str,
        url: &str,
        start_pos: u64,
        size: usize,
    ) -> Result<Bytes> {
        use reqwest::header::RANGE;

        let url = if let Ok(download_url) = ::url::Url::parse(url) {
            let expires = download_url.query_pairs().find_map(|(k, v)| {
                if k == "x-oss-expires" {
                    if let Ok(expires) = v.parse::<u64>() {
                        return Some(expires);
                    }
                }
                None
            });
            if let Some(expires) = expires {
                let current_ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs();
                if current_ts >= expires {
                    debug!(url = %url, "download url expired");
                    self.get_download_url(file_id).await?
                } else {
                    url.to_string()
                }
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        };
        let end_pos = start_pos + size as u64 - 1;
        debug!(url = %url, start = start_pos, end = end_pos, "download file");
        let range = format!("bytes={}-{}", start_pos, end_pos);
        let res = self
            .client
            .get(url)
            .header(RANGE, range)
            .send()
            .await?
            .error_for_status()?;
        Ok(res.bytes().await?)
    }

    pub async fn get_download_url(&self, file_id: &str) -> Result<String> {
        debug!(file_id = %file_id, "get download url");
        let req = GetFileDownloadUrlRequest {
            drive_id: self.drive_id()?,
            file_id,
        };
        let res: GetFileDownloadUrlResponse = self
            .request(format!("{}/v2/file/get_download_url", API_BASE_URL), &req)
            .await?
            .context("expect response")?;
        Ok(res.url)
    }

    pub async fn trash(&self, file_id: &str) -> Result<()> {
        debug!(file_id = %file_id, "trash file");
        let req = TrashRequest {
            drive_id: self.drive_id()?,
            file_id,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v2/recyclebin/trash", API_BASE_URL), &req)
            .await?;
        Ok(())
    }

    pub async fn create_folder(&self, parent_file_id: &str, name: &str) -> Result<()> {
        debug!(parent_file_id = %parent_file_id, name = %name, "create folder");
        let req = CreateFolderRequest {
            check_name_mode: "refuse",
            drive_id: self.drive_id()?,
            name,
            parent_file_id,
            r#type: "folder",
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(
                format!("{}/adrive/v2/file/createWithFolders", API_BASE_URL),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn rename_file(&self, file_id: &str, name: &str) -> Result<()> {
        debug!(file_id = %file_id, name = %name, "rename file");
        let req = RenameFileRequest {
            check_name_mode: "refuse",
            drive_id: self.drive_id()?,
            file_id,
            name,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v3/file/update", API_BASE_URL), &req)
            .await?;
        Ok(())
    }

    pub async fn move_file(&self, file_id: &str, to_parent_file_id: &str) -> Result<()> {
        debug!(file_id = %file_id, to_parent_file_id = %to_parent_file_id, "move file");
        let drive_id = self.drive_id()?;
        let req = MoveFileRequest {
            drive_id,
            file_id,
            to_drive_id: drive_id,
            to_parent_file_id,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v3/file/move", API_BASE_URL), &req)
            .await?;
        Ok(())
    }

    pub async fn create_file_with_proof(
        &self,
        name: &str,
        parent_file_id: &str,
        size: u64,
    ) -> Result<CreateFileWithProofResponse> {
        debug!(name = %name, parent_file_id = %parent_file_id, size = size, "create file with proof");
        let drive_id = self.drive_id()?;
        let chunk_count =
            size / UPLOAD_CHUNK_SIZE + if size % UPLOAD_CHUNK_SIZE != 0 { 1 } else { 0 };
        let part_info_list = (1..=chunk_count)
            .map(|part_number| PartInfo {
                part_number,
                upload_url: String::new(),
            })
            .collect();
        let req = CreateFileWithProofRequest {
            check_name_mode: "refuse",
            content_hash: "",
            content_hash_name: "none",
            drive_id,
            name,
            parent_file_id,
            proof_code: "",
            proof_version: "v1",
            size,
            part_info_list,
            r#type: "file",
        };
        let res: CreateFileWithProofResponse = self
            .request(format!("{}/v2/file/create_with_proof", API_BASE_URL), &req)
            .await?
            .context("expect response")?;
        Ok(res)
    }

    pub async fn complete_file_upload(&self, file_id: &str, upload_id: &str) -> Result<()> {
        debug!(file_id = %file_id, upload_id = %upload_id, "complete file upload");
        let drive_id = self.drive_id()?;
        let req = CompleteUploadRequest {
            drive_id,
            file_id,
            upload_id,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v2/file/complete", API_BASE_URL), &req)
            .await?;
        Ok(())
    }

    pub async fn upload(&self, url: &str, body: Bytes) -> Result<()> {
        self.client
            .put(url)
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
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

#[derive(Debug, Clone, Serialize)]
struct GetFileDownloadUrlRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
struct GetFileDownloadUrlResponse {
    url: String,
    size: u64,
    expiration: String,
}

#[derive(Debug, Clone, Serialize)]
struct TrashRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct CreateFolderRequest<'a> {
    check_name_mode: &'a str,
    drive_id: &'a str,
    name: &'a str,
    parent_file_id: &'a str,
    r#type: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct RenameFileRequest<'a> {
    check_name_mode: &'a str,
    drive_id: &'a str,
    file_id: &'a str,
    name: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct MoveFileRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    to_drive_id: &'a str,
    to_parent_file_id: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartInfo {
    pub part_number: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub upload_url: String,
}

#[derive(Debug, Clone, Serialize)]
struct CreateFileWithProofRequest<'a> {
    check_name_mode: &'a str,
    content_hash: &'a str,
    content_hash_name: &'a str,
    drive_id: &'a str,
    name: &'a str,
    parent_file_id: &'a str,
    proof_code: &'a str,
    proof_version: &'a str,
    size: u64,
    part_info_list: Vec<PartInfo>,
    r#type: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateFileWithProofResponse {
    #[serde(default)]
    pub part_info_list: Vec<PartInfo>,
    pub file_id: String,
    pub upload_id: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct CompleteUploadRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    upload_id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct GetUploadUrlRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    upload_id: &'a str,
    part_info_list: Vec<PartInfo>,
}

#[derive(Debug, Clone)]
pub struct DateTime(SystemTime);

impl DateTime {
    pub fn new(st: SystemTime) -> Self {
        Self(st)
    }
}

impl<'a> Deserialize<'a> for DateTime {
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        let dt = OffsetDateTime::parse(<&str>::deserialize(deserializer)?, &Rfc3339)
            .map_err(serde::de::Error::custom)?;
        Ok(Self(dt.into()))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Folder,
    File,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AliyunFile {
    pub name: String,
    #[serde(rename = "file_id")]
    pub id: String,
    pub r#type: FileType,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    #[serde(default)]
    pub size: u64,
}

impl AliyunFile {
    pub fn new_root() -> Self {
        let now = SystemTime::now();
        Self {
            name: "/".to_string(),
            id: "root".to_string(),
            r#type: FileType::Folder,
            created_at: DateTime(now),
            updated_at: DateTime(now),
            size: 0,
        }
    }
}

impl DavMetaData for AliyunFile {
    fn len(&self) -> u64 {
        self.size
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(self.updated_at.0)
    }

    fn is_dir(&self) -> bool {
        matches!(self.r#type, FileType::Folder)
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(self.created_at.0)
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
