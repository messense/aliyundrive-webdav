use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
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

#[derive(Debug, Clone)]
pub struct DriveConfig {
    pub api_base_url: String,
    pub refresh_token_url: String,
    pub workdir: Option<PathBuf>,
    pub app_id: Option<String>,
    pub client_type: ClientType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClientType {
    Web,
    App,
}

impl ClientType {
    fn refresh_token_url(&self) -> &'static str {
        match self {
            ClientType::Web => "https://api.aliyundrive.com/token/refresh",
            ClientType::App => "https://auth.aliyundrive.com/v2/account/token",
        }
    }
}

impl FromStr for ClientType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "web" | "" => Ok(ClientType::Web),
            "app" => Ok(ClientType::App),
            _ => bail!("invalid client type '{}'", s),
        }
    }
}

impl fmt::Display for ClientType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientType::Web => write!(f, "web"),
            ClientType::App => write!(f, "app"),
        }
    }
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
        if let Some(dir) = self.config.workdir.as_ref() {
            tokio::fs::create_dir_all(dir).await?;
            let refresh_token_file = dir.join("refresh_token");
            tokio::fs::write(refresh_token_file, refresh_token).await?;
        }
        Ok(())
    }

    async fn do_refresh_token(
        &self,
        refresh_token: &str,
        client_type: ClientType,
    ) -> Result<RefreshTokenResponse> {
        let mut data = HashMap::new();
        data.insert("refresh_token", refresh_token);
        data.insert("grant_type", "refresh_token");
        if let Some(app_id) = self.config.app_id.as_ref() {
            data.insert("app_id", app_id);
        }
        let refresh_token_url = if self.config.app_id.is_none() {
            client_type.refresh_token_url()
        } else {
            &self.config.refresh_token_url
        };
        let res = self
            .client
            .post(refresh_token_url)
            .json(&data)
            .send()
            .await?;
        match res.error_for_status_ref() {
            Ok(_) => {
                let res = res.json::<RefreshTokenResponse>().await?;
                info!(
                    refresh_token = %res.refresh_token,
                    nick_name = %res.nick_name,
                    "refresh token succeed"
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
        let mut client_type = self.config.client_type;
        for _ in 0..10 {
            match self.do_refresh_token(&refresh_token, client_type).await {
                Ok(res) => {
                    let mut cred = self.credentials.write().await;
                    cred.refresh_token = res.refresh_token.clone();
                    cred.access_token = Some(res.access_token.clone());
                    let save_content = format!("{}:{}", client_type, res.refresh_token);
                    if let Err(err) = self.save_refresh_token(&save_content).await {
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
                            (refresh_token, client_type) =
                                parse_refresh_token(refresh_token_from_file.trim())?;
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

    pub async fn get_file(&self, file_id: &str) -> Result<Option<AliyunFile>> {
        let drive_id = self.drive_id()?;
        debug!(drive_id = %drive_id, file_id = %file_id, "get file");
        let req = GetFileRequest { drive_id, file_id };
        let res: Result<GetFileResponse> = self
            .request(format!("{}/v2/file/get", self.config.api_base_url), &req)
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
                format!("{}/v2/file/get_by_path", self.config.api_base_url),
                &req,
            )
            .await
            .and_then(|res| res.context("expect response"));
        match res {
            Ok(file) => Ok(Some(file)),
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
            all: false,
            image_thumbnail_process: "image/resize,w_400/format,jpeg",
            image_url_process: "image/resize,w_1920/format,jpeg",
            video_thumbnail_process: "video/snapshot,t_0,f_jpg,ar_auto,w_300",
            fields: "*",
            order_by: "updated_at",
            order_direction: "DESC",
            marker,
        };
        self.request(format!("{}/v2/file/list", self.config.api_base_url), &req)
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
        };
        let res: GetFileDownloadUrlResponse = self
            .request(
                format!("{}/v2/file/get_download_url", self.config.api_base_url),
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
                format!("{}/v2/recyclebin/trash", self.config.api_base_url),
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
            .request(format!("{}/v2/file/delete", self.config.api_base_url), &req)
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
            .request(format!("{}/v2/file/create", self.config.api_base_url), &req)
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
            .request(format!("{}/v2/file/update", self.config.api_base_url), &req)
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
            to_drive_id: drive_id,
            to_parent_file_id,
            new_name,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v2/file/move", self.config.api_base_url), &req)
            .await?;
        Ok(())
    }

    pub async fn copy_file(
        &self,
        file_id: &str,
        to_parent_file_id: &str,
        new_name: Option<&str>,
    ) -> Result<()> {
        debug!(file_id = %file_id, to_parent_file_id = %to_parent_file_id, "copy file");
        let drive_id = self.drive_id()?;
        let req = CopyFileRequest {
            drive_id,
            file_id,
            to_parent_file_id,
            new_name,
        };
        let _res: Option<serde::de::IgnoredAny> = self
            .request(format!("{}/v2/file/copy", self.config.api_base_url), &req)
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
                format!("{}/v2/file/create_with_proof", self.config.api_base_url),
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
                format!("{}/v2/file/complete", self.config.api_base_url),
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
                format!("{}/v2/file/get_upload_url", self.config.api_base_url),
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
        let res: GetDriveResponse = self
            .request(format!("{}/v2/drive/get", self.config.api_base_url), &data)
            .await?
            .context("expect response")?;
        Ok((res.used_size, res.total_size))
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
    Ok(tokio::fs::read_to_string(workdir.join("refresh_token")).await?)
}

pub fn parse_refresh_token(content: &str) -> Result<(String, ClientType)> {
    let (client, token) = content
        .trim()
        .split_once(':')
        .unwrap_or_else(|| ("app", content.trim()));
    Ok((token.to_string(), client.parse()?))
}
