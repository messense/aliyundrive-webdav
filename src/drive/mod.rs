use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use clap::ValueEnum;
use dav_server::fs::{DavDirEntry, DavMetaData, FsFuture, FsResult};
use futures_util::future::FutureExt;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    IntoUrl, StatusCode,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::{
    sync::{oneshot, RwLock},
    time,
};
use tracing::{debug, error, info, warn};

pub mod model;

use model::*;
pub use model::{AliyunFile, DateTime, FileType};

const ORIGIN: &str = "https://www.aliyundrive.com";
const REFERER: &str = "https://www.aliyundrive.com/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.83 Safari/537.36";

/// Aliyundrive drive type
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DriveType {
    /// Resource drive
    Resource,
    /// Backup drive
    Backup,
    /// Default drive
    Default,
}

#[derive(Debug, Clone)]
pub struct DriveConfig {
    pub api_base_url: String,
    pub refresh_token_host: String,
    pub workdir: Option<PathBuf>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub drive_type: Option<DriveType>,
}

#[derive(Debug, Clone)]
struct Credentials {
    refresh_token: String,
    access_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AliyunDrive {
    config: DriveConfig,
    client: ClientWithMiddleware,
    credentials: Arc<RwLock<Credentials>>,
    drive_id: Option<String>,
}

impl AliyunDrive {
    pub async fn new(config: DriveConfig, refresh_token: String) -> Result<Self> {
        let refresh_token_is_empty = refresh_token.is_empty();
        let credentials = Credentials {
            refresh_token,
            access_token: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));
        if let Ok(canary_env) = std::env::var("ALIYUNDRIVE_CANARY") {
            // 灰度环境：gray
            headers.insert("X-Canary", HeaderValue::from_str(&canary_env)?);
        }
        let retry_policy = ExponentialBackoff::builder()
            .backoff_exponent(2)
            .retry_bounds(Duration::from_millis(100), Duration::from_secs(5))
            .build_with_max_retries(3);
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            // OSS closes idle connections after 60 seconds,
            // so we can close idle connections ahead of time to prevent re-using them.
            // See also https://github.com/hyperium/hyper/issues/2136
            .pool_idle_timeout(Duration::from_secs(50))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let drive_type = config.drive_type.clone();
        let mut drive = Self {
            config,
            client,
            credentials: Arc::new(RwLock::new(credentials)),
            drive_id: None,
        };

        let (tx, rx) = oneshot::channel();
        // schedule update token task
        let refresh_token_from_file = if let Some(dir) = drive.config.workdir.as_ref() {
            read_refresh_token(dir).await.ok()
        } else {
            None
        };
        if refresh_token_is_empty && refresh_token_from_file.is_none() {
            bail!("No refresh token provided! \n📝 Please specify refresh token from `--refresh-token` CLI option.");
        }

        let client = drive.clone();
        tokio::spawn(async move {
            let mut delay_seconds = 7000;
            match client
                .do_refresh_token_with_retry(refresh_token_from_file)
                .await
            {
                Ok(res) => {
                    // token usually expires in 7200s, refresh earlier
                    delay_seconds = res.expires_in - 200;
                    if tx.send(res.access_token).is_err() {
                        error!("send access_token failed");
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

        let access_token = rx.await?;
        if access_token.is_empty() {
            bail!("get access_token failed");
        }
        let drive_type_str = match drive_type {
            Some(DriveType::Resource) => "resource",
            Some(DriveType::Backup) => "backup",
            Some(DriveType::Default) | None => "default",
        };
        let drive_id = drive
            .get_drive_id(drive_type)
            .await
            .context("get drive id failed")?;
        info!(drive_id = %drive_id, "found {} drive", drive_type_str);
        drive.drive_id = Some(drive_id);

        Ok(drive)
    }

    async fn save_refresh_token(&self, refresh_token: &str) -> Result<()> {
        if let Some(dir) = self.config.workdir.as_ref() {
            tokio::fs::create_dir_all(dir).await?;
            let refresh_token_file = dir.join("refresh_token");
            tokio::fs::write(refresh_token_file, refresh_token).await?;
        }
        Ok(())
    }

    async fn do_refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenResponse> {
        let mut data = HashMap::new();
        data.insert("refresh_token", refresh_token);
        data.insert("grant_type", "refresh_token");
        if let Some(client_id) = self.config.client_id.as_ref() {
            data.insert("client_id", client_id);
        }
        if let Some(client_secret) = self.config.client_secret.as_ref() {
            data.insert("client_secret", client_secret);
        }
        let res = self
            .client
            .post(format!(
                "{}/oauth/access_token",
                &self.config.refresh_token_host
            ))
            .json(&data)
            .send()
            .await?;
        match res.error_for_status_ref() {
            Ok(_) => {
                let res = res.json::<RefreshTokenResponse>().await?;
                info!("refresh token succeed");
                debug!(
                    refresh_token = %res.refresh_token,
                    "new refresh token"
                );
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
                Ok(res) => {
                    let mut cred = self.credentials.write().await;
                    cred.refresh_token = res.refresh_token.clone();
                    cred.access_token = Some(res.access_token.clone());
                    if let Err(err) = self.save_refresh_token(&res.refresh_token).await {
                        error!(error = %err, "save refresh token failed");
                    }
                    return Ok(res);
                }
                Err(err) => {
                    let mut should_warn = true;
                    let mut should_retry = match err.downcast_ref::<reqwest::Error>() {
                        Some(e) => {
                            e.is_connect()
                                || e.is_timeout()
                                || matches!(e.status(), Some(StatusCode::TOO_MANY_REQUESTS))
                        }
                        None => false,
                    };
                    // retry if command line refresh_token is invalid but we also have
                    // refresh_token from file
                    if let Some(refresh_token_from_file) = refresh_token_from_file.as_ref() {
                        if !should_retry && &refresh_token != refresh_token_from_file {
                            refresh_token = refresh_token_from_file.trim().to_string();
                            should_retry = true;
                            // don't warn if we are gonna try refresh_token from file
                            should_warn = false;
                        }
                    }
                    if should_retry {
                        if should_warn {
                            warn!(error = %err, "refresh token failed, will wait and retry");
                        }
                        last_err = Some(err);
                        time::sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        last_err = Some(err);
                        break;
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
        cred.access_token.clone().context("missing access_token")
    }

    fn drive_id(&self) -> Result<&str> {
        self.drive_id.as_deref().context("missing drive_id")
    }

    async fn request<T, U>(&self, url: String, req: &T) -> Result<Option<U>>
    where
        T: Serialize + ?Sized,
        U: DeserializeOwned,
    {
        let mut access_token = self.access_token().await?;
        let url = reqwest::Url::parse(&url)?;
        let res = self
            .client
            .post(url.clone())
            .bearer_auth(&access_token)
            .json(&req)
            .send()
            .await?;
        match res.error_for_status_ref() {
            Ok(_) => {
                if res.status() == StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                // let res = res.text().await?;
                // println!("{}: {}", url, res);
                // let res = serde_json::from_str(&res)?;
                let res = res.json::<U>().await?;
                Ok(Some(res))
            }
            Err(err) => {
                let err_msg = res.text().await?;
                debug!(error = %err_msg, url = %url, "request failed");
                match err.status() {
                    Some(
                        status_code
                        @
                        // 4xx
                        (StatusCode::UNAUTHORIZED
                        | StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_MANY_REQUESTS
                        // 5xx
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

    pub async fn get_drive_id(&self, drive_type: Option<DriveType>) -> Result<String> {
        let req = HashMap::<String, String>::new();
        let res: GetDriveInfoResponse = self
            .request(
                format!("{}/adrive/v1.0/user/getDriveInfo", self.config.api_base_url),
                &req,
            )
            .await
            .and_then(|res| res.context("expect response"))?;
        let drive_id = match drive_type {
            Some(DriveType::Resource) => res.resource_drive_id.unwrap_or_else(|| {
                warn!("resource drive not found, use default drive instead");
                res.default_drive_id
            }),
            Some(DriveType::Backup) => res.backup_drive_id.unwrap_or_else(|| {
                warn!("backup drive not found, use default drive instead");
                res.default_drive_id
            }),
            Some(DriveType::Default) | None => res.default_drive_id,
        };
        Ok(drive_id)
    }

    pub async fn get_file(&self, file_id: &str) -> Result<Option<AliyunFile>> {
        let drive_id = self.drive_id()?;
        debug!(drive_id = %drive_id, file_id = %file_id, "get file");
        let req = GetFileRequest { drive_id, file_id };
        let res: Result<GetFileResponse> = self
            .request(
                format!("{}/adrive/v1.0/openFile/get", self.config.api_base_url),
                &req,
            )
            .await
            .and_then(|res| res.context("expect response"));
        match res {
            Ok(file) => Ok(Some(file.into())),
            Err(err) => {
                if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
                    if matches!(req_err.status(), Some(StatusCode::NOT_FOUND)) {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                } else {
                    Err(err)
                }
            }
        }
    }

    pub async fn get_by_path(&self, path: &str) -> Result<Option<AliyunFile>> {
        let drive_id = self.drive_id()?;
        debug!(drive_id = %drive_id, path = %path, "get file by path");
        if path == "/" || path.is_empty() {
            return Ok(Some(AliyunFile::new_root()));
        }
        let req = GetFileByPathRequest {
            drive_id,
            file_path: path,
        };
        let res: Result<AliyunFile> = self
            .request(
                format!(
                    "{}/adrive/v1.0/openFile/get_by_path",
                    self.config.api_base_url
                ),
                &req,
            )
            .await
            .and_then(|res| res.context("expect response"));
        match res {
            Ok(file) => Ok(Some(file)),
            Err(_) => Ok(None),
        }
    }

    pub async fn list_all(&self, parent_file_id: &str) -> Result<Vec<AliyunFile>> {
        let mut files = Vec::new();
        let mut marker = None;
        loop {
            let res = self.list(parent_file_id, marker.as_deref()).await?;
            files.extend(res.items.into_iter().map(|f| f.into()));
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
            fields: "*",
            order_by: "updated_at",
            order_direction: "DESC",
            marker,
        };
        self.request(
            format!("{}/adrive/v1.0/openFile/list", self.config.api_base_url),
            &req,
        )
        .await
        .and_then(|res| res.context("expect response"))
    }

    pub async fn download<U: IntoUrl>(&self, url: U, range: Option<(u64, usize)>) -> Result<Bytes> {
        use reqwest::header::RANGE;

        let url = url.into_url()?;
        let res = if let Some((start_pos, size)) = range {
            let end_pos = start_pos + size as u64 - 1;
            debug!(url = %url, start = start_pos, end = end_pos, "download file");
            let range = format!("bytes={}-{}", start_pos, end_pos);
            self.client
                .get(url)
                .header(RANGE, range)
                .send()
                .await?
                .error_for_status()?
        } else {
            debug!(url = %url, "download file");
            self.client.get(url).send().await?.error_for_status()?
        };
        Ok(res.bytes().await?)
    }

    pub async fn get_download_url(&self, file_id: &str) -> Result<GetFileDownloadUrlResponse> {
        debug!(file_id = %file_id, "get download url");
        let req = GetFileDownloadUrlRequest {
            drive_id: self.drive_id()?,
            file_id,
            expire_sec: 14400, // 4 hours
        };
        let res: GetFileDownloadUrlResponse = self
            .request(
                format!(
                    "{}/adrive/v1.0/openFile/getDownloadUrl",
                    self.config.api_base_url
                ),
                &req,
            )
            .await?
            .context("expect response")?;
        Ok(res)
    }

    async fn trash(&self, file_id: &str) -> Result<()> {
        debug!(file_id = %file_id, "trash file");
        let req = TrashRequest {
            drive_id: self.drive_id()?,
            file_id,
        };
        let res: Result<Option<serde::de::IgnoredAny>> = self
            .request(
                format!(
                    "{}/adrive/v1.0/openFile/recyclebin/trash",
                    self.config.api_base_url
                ),
                &req,
            )
            .await;
        if let Err(err) = res {
            if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
                // Ignore 404 and 400 status codes
                if !matches!(
                    req_err.status(),
                    Some(StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST)
                ) {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    async fn delete_file(&self, file_id: &str) -> Result<()> {
        debug!(file_id = %file_id, "delete file");
        let req = TrashRequest {
            drive_id: self.drive_id()?,
            file_id,
        };
        let res: Result<Option<serde::de::IgnoredAny>> = self
            .request(
                format!("{}/adrive/v1.0/openFile/delete", self.config.api_base_url),
                &req,
            )
            .await;
        if let Err(err) = res {
            if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
                // Ignore 404 and 400 status codes
                if !matches!(
                    req_err.status(),
                    Some(StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST)
                ) {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub async fn remove_file(&self, file_id: &str, trash: bool) -> Result<()> {
        if trash {
            self.trash(file_id).await?;
        } else {
            self.delete_file(file_id).await?;
        }
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
                format!("{}/adrive/v1.0/openFile/create", self.config.api_base_url),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn rename_file(&self, file_id: &str, name: &str) -> Result<()> {
        debug!(file_id = %file_id, name = %name, "rename file");
        let req = RenameFileRequest {
            drive_id: self.drive_id()?,
            file_id,
            name,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(
                format!("{}/adrive/v1.0/openFile/update", self.config.api_base_url),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn move_file(
        &self,
        file_id: &str,
        to_parent_file_id: &str,
        new_name: Option<&str>,
    ) -> Result<()> {
        debug!(file_id = %file_id, to_parent_file_id = %to_parent_file_id, "move file");
        let drive_id = self.drive_id()?;
        let req = MoveFileRequest {
            drive_id,
            file_id,
            to_parent_file_id,
            new_name,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(
                format!("{}/adrive/v1.0/openFile/move", self.config.api_base_url),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn copy_file(&self, file_id: &str, to_parent_file_id: &str) -> Result<()> {
        debug!(file_id = %file_id, to_parent_file_id = %to_parent_file_id, "copy file");
        let drive_id = self.drive_id()?;
        let req = CopyFileRequest {
            drive_id,
            file_id,
            to_parent_file_id,
            auto_rename: false,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(
                format!("{}/adrive/v1.0/openFile/copy", self.config.api_base_url),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn create_file_with_proof(
        &self,
        name: &str,
        parent_file_id: &str,
        size: u64,
        chunk_count: u64,
    ) -> Result<CreateFileWithProofResponse> {
        debug!(name = %name, parent_file_id = %parent_file_id, size = size, "create file with proof");
        let drive_id = self.drive_id()?;
        let part_info_list = (1..=chunk_count)
            .map(|part_number| UploadPartInfo {
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
            .request(
                format!("{}/adrive/v1.0/openFile/create", self.config.api_base_url),
                &req,
            )
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
            .request(
                format!("{}/adrive/v1.0/openFile/complete", self.config.api_base_url),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn upload(&self, url: &str, body: Bytes) -> Result<()> {
        let res = self.client.put(url).body(body).send().await?;
        if let Err(err) = res.error_for_status_ref() {
            let detail = res
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            bail!("{}: {}", err, detail);
        }
        Ok(())
    }

    pub async fn get_upload_url(
        &self,
        file_id: &str,
        upload_id: &str,
        chunk_count: u64,
    ) -> Result<Vec<UploadPartInfo>> {
        debug!(file_id = %file_id, upload_id = %upload_id, "get upload url");
        let drive_id = self.drive_id()?;
        let part_info_list = (1..=chunk_count)
            .map(|part_number| UploadPartInfo {
                part_number,
                upload_url: String::new(),
            })
            .collect();
        let req = GetUploadUrlRequest {
            drive_id,
            file_id,
            upload_id,
            part_info_list,
        };
        let res: CreateFileWithProofResponse = self
            .request(
                format!(
                    "{}/adrive/v1.0/openFile/getUploadUrl",
                    self.config.api_base_url
                ),
                &req,
            )
            .await?
            .context("expect response")?;
        Ok(res.part_info_list)
    }

    pub async fn get_quota(&self) -> Result<(u64, u64)> {
        let drive_id = self.drive_id()?;
        let mut data = HashMap::new();
        data.insert("drive_id", drive_id);
        let res: GetSpaceInfoResponse = self
            .request(
                format!("{}/adrive/v1.0/user/getSpaceInfo", self.config.api_base_url),
                &data,
            )
            .await?
            .context("expect response")?;
        Ok((
            res.personal_space_info.used_size,
            res.personal_space_info.total_size,
        ))
    }
}

impl DavMetaData for AliyunFile {
    fn len(&self) -> u64 {
        self.size
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(*self.updated_at)
    }

    fn is_dir(&self) -> bool {
        matches!(self.r#type, FileType::Folder)
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(*self.created_at)
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

pub async fn read_refresh_token(workdir: &Path) -> Result<String> {
    let file = workdir.join("refresh_token");
    let token = tokio::fs::read_to_string(&file).await?;
    if token.split('.').count() < 3 {
        bail!(
            "Please remove outdated refresh_token cache for v1.x at {}",
            file.display(),
        );
    }
    Ok(token)
}
