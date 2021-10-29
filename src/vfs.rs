use std::fmt::{Debug, Formatter};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use dashmap::DashMap;
use futures_util::future::FutureExt;
use tracing::{debug, error, trace};
use webdav_handler::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
        ReadDirMeta,
    },
};

use crate::{
    cache::Cache,
    drive::{AliyunDrive, AliyunFile, DateTime, FileType},
};

const UPLOAD_CHUNK_SIZE: u64 = 16 * 1024 * 1024; // 16MB

#[derive(Clone)]
pub struct AliyunDriveFileSystem {
    drive: AliyunDrive,
    dir_cache: Cache,
    uploading: Arc<DashMap<String, Vec<AliyunFile>>>,
    root: PathBuf,
    no_trash: bool,
}

impl AliyunDriveFileSystem {
    pub async fn new(
        drive: AliyunDrive,
        root: String,
        cache_size: usize,
        cache_ttl: u64,
        no_trash: bool,
    ) -> Result<Self> {
        let dir_cache = Cache::new(cache_size, cache_ttl);
        debug!("dir cache initialized");
        let root = if root.starts_with('/') {
            PathBuf::from(root)
        } else {
            Path::new("/").join(root)
        };
        Ok(Self {
            drive,
            dir_cache,
            uploading: Arc::new(DashMap::new()),
            root,
            no_trash,
        })
    }

    fn find_in_cache(&self, path: &Path) -> Result<Option<AliyunFile>, FsError> {
        if let Some(parent) = path.parent() {
            let parent_str = parent.to_string_lossy().into_owned();
            let file_name = path
                .file_name()
                .ok_or(FsError::NotFound)?
                .to_string_lossy()
                .into_owned();
            let file = self.dir_cache.get(&parent_str).and_then(|files| {
                for file in &files {
                    if file.name == file_name {
                        return Some(file.clone());
                    }
                }
                None
            });
            Ok(file)
        } else {
            let root = AliyunFile::new_root();
            Ok(Some(root))
        }
    }

    async fn get_file(&self, path: PathBuf) -> Result<Option<AliyunFile>, FsError> {
        let path_str = path.to_string_lossy().into_owned();
        let file = self.find_in_cache(&path)?;
        if let Some(file) = file {
            trace!(path = %path.display(), file_id = %file.id, "file found in cache");
            Ok(Some(file))
        } else {
            trace!(path = %path.display(), "file not found in cache");
            if let Ok(Some(file)) = self.drive.get_by_path(&path_str).await {
                return Ok(Some(file));
            }

            // path may contain whitespaces which get_by_path can't handle
            // so we try to find it in directory
            let parts: Vec<&str> = path_str.split('/').collect();
            let parts_len = parts.len();
            let filename = parts[parts_len - 1];
            let mut prefix = PathBuf::from("/");
            for part in &parts[0..parts_len - 1] {
                let parent = prefix.join(part);
                prefix = parent.clone();
                let files = self.read_dir_and_cache(parent).await?;
                if let Some(file) = files.iter().find(|f| f.name == filename) {
                    trace!(path = %path.display(), file_id = %file.id, "file found in cache");
                    return Ok(Some(file.clone()));
                }
            }
            Ok(None)
        }
    }

    async fn read_dir_and_cache(&self, path: PathBuf) -> Result<Vec<AliyunFile>, FsError> {
        let path_str = path.to_string_lossy().into_owned();
        debug!(path = %path_str, "read_dir and cache");
        let parent_file_id = if path_str == "/" {
            "root".to_string()
        } else {
            match self.find_in_cache(&path) {
                Ok(Some(file)) => file.id,
                _ => {
                    if let Ok(Some(file)) = self.drive.get_by_path(&path_str).await {
                        file.id
                    } else {
                        return Err(FsError::NotFound);
                    }
                }
            }
        };
        let mut files = if let Some(files) = self.dir_cache.get(&path_str) {
            files
        } else {
            self.list_files_and_cache(path_str, parent_file_id.clone())
                .await
                .map_err(|_| FsError::NotFound)?
        };
        let uploading_files = self.list_uploading_files(&parent_file_id);
        if !uploading_files.is_empty() {
            debug!("added {} uploading files", uploading_files.len());
        }
        files.extend(uploading_files);
        Ok(files)
    }

    fn list_uploading_files(&self, parent_file_id: &str) -> Vec<AliyunFile> {
        self.uploading
            .get(parent_file_id)
            .map(|val_ref| val_ref.value().clone())
            .unwrap_or_default()
    }

    fn remove_uploading_file(&self, parent_file_id: &str, name: &str) {
        if let Some(mut files) = self.uploading.get_mut(parent_file_id) {
            if let Some(index) = files.iter().position(|x| x.name == name) {
                files.swap_remove(index);
            }
        }
    }

    async fn list_files_and_cache(
        &self,
        path_str: String,
        parent_file_id: String,
    ) -> Result<Vec<AliyunFile>> {
        let files = self.drive.list_all(&parent_file_id).await?;
        self.cache_dir(path_str, files.clone()).await;
        Ok(files)
    }

    async fn cache_dir(&self, dir_path: String, files: Vec<AliyunFile>) {
        trace!(path = %dir_path, count = files.len(), "cache dir");
        self.dir_cache.insert(dir_path, files).await;
    }

    fn normalize_dav_path(&self, dav_path: &DavPath) -> PathBuf {
        let path = dav_path.as_pathbuf();
        if self.root.parent().is_none() || path.starts_with(&self.root) {
            return path;
        }
        let rel_path = dav_path.as_rel_ospath();
        if rel_path == Path::new("") {
            return self.root.clone();
        }
        self.root.join(rel_path)
    }
}

impl DavFileSystem for AliyunDriveFileSystem {
    fn open<'a>(
        &'a self,
        dav_path: &'a DavPath,
        options: OpenOptions,
    ) -> FsFuture<Box<dyn DavFile>> {
        let path = self.normalize_dav_path(dav_path);
        let mode = if options.write { "write" } else { "read" };
        debug!(path = %path.display(), mode = %mode, "fs: open");
        async move {
            if options.append {
                // Can't support open in write-append mode
                error!(path = %path.display(), "unsupported write-append mode");
                return Err(FsError::NotImplemented);
            }
            let parent_path = path.parent().ok_or(FsError::NotFound)?;
            let parent_file = self
                .get_file(parent_path.to_path_buf())
                .await?
                .ok_or(FsError::NotFound)?;
            let dav_file = if let Some(mut file) = self.get_file(path.clone()).await? {
                if options.write && options.create_new {
                    return Err(FsError::Exists);
                }
                if let Some(size) = options.size {
                    // 上传中的文件刚开始 size 可能为 0，更新为正确的 size
                    if file.size == 0 {
                        file.size = size;
                    }
                }
                AliyunDavFile::new(self.clone(), file, parent_file.id)
            } else if options.write && (options.create || options.create_new) {
                let size = options.size;
                let name = dav_path
                    .file_name()
                    .ok_or(FsError::GeneralFailure)?
                    .to_string();

                // 忽略 macOS 上的一些特殊文件
                if name == ".DS_Store" || name.starts_with("._") {
                    return Err(FsError::NotFound);
                }

                let now = SystemTime::now();
                let file = AliyunFile {
                    name,
                    id: "".to_string(),
                    r#type: FileType::File,
                    created_at: DateTime::new(now),
                    updated_at: DateTime::new(now),
                    size: size.unwrap_or(0),
                };
                let mut uploading = self.uploading.entry(parent_file.id.clone()).or_default();
                uploading.push(file.clone());
                AliyunDavFile::new(self.clone(), file, parent_file.id)
            } else {
                return Err(FsError::NotFound);
            };
            Ok(Box::new(dav_file) as Box<dyn DavFile>)
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<FsStream<Box<dyn DavDirEntry>>> {
        let path = self.normalize_dav_path(path);
        debug!(path = %path.display(), "fs: read_dir");
        async move {
            let files = self.read_dir_and_cache(path.clone()).await?;
            let mut v: Vec<Box<dyn DavDirEntry>> = Vec::with_capacity(files.len());
            for file in files {
                v.push(Box::new(file));
            }
            let stream = futures_util::stream::iter(v);
            Ok(Box::pin(stream) as FsStream<Box<dyn DavDirEntry>>)
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        let path = self.normalize_dav_path(path);
        debug!(path = %path.display(), "fs: metadata");
        async move {
            let file = self.get_file(path).await?.ok_or(FsError::NotFound)?;
            Ok(Box::new(file) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn create_dir<'a>(&'a self, dav_path: &'a DavPath) -> FsFuture<()> {
        let path = self.normalize_dav_path(dav_path);
        debug!(path = %path.display(), "fs: create_dir");
        async move {
            let parent_path = path.parent().ok_or(FsError::NotFound)?;
            let parent_file = self
                .get_file(parent_path.to_path_buf())
                .await?
                .ok_or(FsError::NotFound)?;
            if !matches!(parent_file.r#type, FileType::Folder) {
                return Err(FsError::Forbidden);
            }
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy().into_owned();
                self.drive
                    .create_folder(&parent_file.id, &name)
                    .await
                    .map_err(|err| {
                        error!(path = %path.display(), error = %err, "create folder failed");
                        FsError::GeneralFailure
                    })?;
                self.dir_cache.invalidate(parent_path).await;
                Ok(())
            } else {
                Err(FsError::Forbidden)
            }
        }
        .boxed()
    }

    fn remove_dir<'a>(&'a self, dav_path: &'a DavPath) -> FsFuture<()> {
        let path = self.normalize_dav_path(dav_path);
        debug!(path = %path.display(), "fs: remove_dir");
        async move {
            let file = self
                .get_file(path.clone())
                .await?
                .ok_or(FsError::NotFound)?;
            if !matches!(file.r#type, FileType::Folder) {
                return Err(FsError::Forbidden);
            }
            self.drive
                .remove_file(&file.id, !self.no_trash)
                .await
                .map_err(|err| {
                    error!(path = %path.display(), error = %err, "remove directory failed");
                    FsError::GeneralFailure
                })?;
            self.dir_cache.invalidate_parent(&path).await;
            Ok(())
        }
        .boxed()
    }

    fn remove_file<'a>(&'a self, dav_path: &'a DavPath) -> FsFuture<()> {
        let path = self.normalize_dav_path(dav_path);
        debug!(path = %path.display(), "fs: remove_file");
        async move {
            let file = self
                .get_file(path.clone())
                .await?
                .ok_or(FsError::NotFound)?;
            if !matches!(file.r#type, FileType::File) {
                return Err(FsError::Forbidden);
            }
            self.drive
                .remove_file(&file.id, !self.no_trash)
                .await
                .map_err(|err| {
                    error!(path = %path.display(), error = %err, "remove file failed");
                    FsError::GeneralFailure
                })?;
            self.dir_cache.invalidate_parent(&path).await;
            Ok(())
        }
        .boxed()
    }

    fn copy<'a>(&'a self, from_dav: &'a DavPath, to_dav: &'a DavPath) -> FsFuture<()> {
        let from = self.normalize_dav_path(from_dav);
        let to = self.normalize_dav_path(to_dav);
        debug!(from = %from.display(), to = %to.display(), "fs: copy");
        async move {
            let file = self
                .get_file(from.clone())
                .await?
                .ok_or(FsError::NotFound)?;
            let to_parent_file = self
                .get_file(to.parent().unwrap().to_path_buf())
                .await?
                .ok_or(FsError::NotFound)?;
            let new_name = to_dav.file_name();
            self.drive
                .copy_file(&file.id, &to_parent_file.id, new_name)
                .await
                .map_err(|err| {
                    error!(from = %from.display(), to = %to.display(), error = %err, "copy file failed");
                    FsError::GeneralFailure
                })?;

            self.dir_cache.invalidate_parent(&from).await;
            self.dir_cache.invalidate_parent(&to).await;
            Ok(())
        }
        .boxed()
    }

    fn rename<'a>(&'a self, from_dav: &'a DavPath, to_dav: &'a DavPath) -> FsFuture<()> {
        let from = self.normalize_dav_path(from_dav);
        let to = self.normalize_dav_path(to_dav);
        debug!(from = %from.display(), to = %to.display(), "fs: rename");
        async move {
            if from.parent() == to.parent() {
                // rename
                if let Some(name) = to.file_name() {
                    let file = self
                        .get_file(from.clone())
                        .await?
                        .ok_or(FsError::NotFound)?;
                    let name = name.to_string_lossy().into_owned();
                    self.drive
                        .rename_file(&file.id, &name)
                        .await
                        .map_err(|err| {
                            error!(from = %from.display(), to = %to.display(), error = %err, "rename file failed");
                            FsError::GeneralFailure
                        })?;
                } else {
                    return Err(FsError::Forbidden);
                }
            } else {
                // move
                let file = self
                    .get_file(from.clone())
                    .await?
                    .ok_or(FsError::NotFound)?;
                let to_parent_file = self
                    .get_file(to.parent().unwrap().to_path_buf())
                    .await?
                    .ok_or(FsError::NotFound)?;
                let new_name = to_dav.file_name();
                self.drive
                    .move_file(&file.id, &to_parent_file.id, new_name)
                    .await
                    .map_err(|err| {
                        error!(from = %from.display(), to = %to.display(), error = %err, "move file failed");
                        FsError::GeneralFailure
                    })?;
            }

            self.dir_cache.invalidate_parent(&from).await;
            self.dir_cache.invalidate_parent(&to).await;
            Ok(())
        }
        .boxed()
    }

    fn get_quota(&self) -> FsFuture<(u64, Option<u64>)> {
        debug!("fs: get_quota");
        async move {
            let (used, total) = self.drive.get_quota().await.map_err(|err| {
                error!(error = %err, "get quota failed");
                FsError::GeneralFailure
            })?;
            Ok((used, Some(total)))
        }
        .boxed()
    }
}

#[derive(Debug, Clone)]
struct UploadState {
    buffer: BytesMut,
    chunk_count: u64,
    chunk: u64,
    upload_id: String,
    upload_urls: Vec<String>,
}

impl Default for UploadState {
    fn default() -> Self {
        Self {
            buffer: BytesMut::new(),
            chunk_count: 0,
            chunk: 1,
            upload_id: String::new(),
            upload_urls: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct AliyunDavFile {
    fs: AliyunDriveFileSystem,
    file: AliyunFile,
    parent_file_id: String,
    current_pos: u64,
    download_url: Option<String>,
    upload_state: UploadState,
}

impl Debug for AliyunDavFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AliyunDavFile")
            .field("file", &self.file)
            .field("parent_file_id", &self.parent_file_id)
            .field("current_pos", &self.current_pos)
            .field("upload_state", &self.upload_state)
            .finish()
    }
}

impl AliyunDavFile {
    fn new(fs: AliyunDriveFileSystem, file: AliyunFile, parent_file_id: String) -> Self {
        Self {
            fs,
            file,
            parent_file_id,
            current_pos: 0,
            download_url: None,
            upload_state: UploadState::default(),
        }
    }

    async fn get_download_url(&self) -> Result<String, FsError> {
        self.fs.drive.get_download_url(&self.file.id).await.map_err(|err| {
            error!(file_id = %self.file.id, file_name = %self.file.name, error = %err, "get download url failed");
            FsError::GeneralFailure
        })
    }

    async fn prepare_for_upload(&mut self) -> Result<(), FsError> {
        if self.upload_state.chunk_count == 0 {
            let size = self.file.size;
            debug!(file_name = %self.file.name, size = size, "prepare for upload");
            if !self.file.id.is_empty() {
                // existing file, delete before upload
                if let Err(err) = self
                    .fs
                    .drive
                    .remove_file(&self.file.id, !self.fs.no_trash)
                    .await
                {
                    error!(file_name = %self.file.name, error = %err, "delete file before upload failed");
                }
            }
            // TODO: create parent folders?
            let chunk_count =
                size / UPLOAD_CHUNK_SIZE + if size % UPLOAD_CHUNK_SIZE != 0 { 1 } else { 0 };
            self.upload_state.chunk_count = chunk_count;
            let res = self
                .fs
                .drive
                .create_file_with_proof(&self.file.name, &self.parent_file_id, size, chunk_count)
                .await
                .map_err(|err| {
                    error!(file_name = %self.file.name, error = %err, "create file with proof failed");
                    FsError::GeneralFailure
                })?;
            self.file.id = res.file_id.clone();
            self.upload_state.upload_id = res.upload_id.clone();
            let upload_urls: Vec<_> = res
                .part_info_list
                .into_iter()
                .map(|x| x.upload_url)
                .collect();
            if upload_urls.is_empty() {
                error!(file_id = %self.file.id, file_name = %self.file.name, "empty upload urls");
                return Err(FsError::GeneralFailure);
            }
            self.upload_state.upload_urls = upload_urls;
        }
        Ok(())
    }

    async fn maybe_upload_chunk(&mut self, remaining: bool) -> Result<(), FsError> {
        let chunk_size = if remaining {
            // last chunk size maybe less than UPLOAD_CHUNK_SIZE
            self.upload_state.buffer.remaining()
        } else {
            UPLOAD_CHUNK_SIZE as usize
        };
        let current_chunk = self.upload_state.chunk;
        if chunk_size > 0
            && self.upload_state.buffer.remaining() >= chunk_size
            && current_chunk <= self.upload_state.chunk_count
        {
            let chunk_data = self.upload_state.buffer.split_to(chunk_size);
            debug!(
                file_id = %self.file.id,
                file_name = %self.file.name,
                size = self.file.size,
                "upload part {}/{}",
                current_chunk,
                self.upload_state.chunk_count
            );
            let mut upload_url = &self.upload_state.upload_urls[current_chunk as usize - 1];
            if is_url_expired(upload_url) {
                if let Ok(part_info_list) = self
                    .fs
                    .drive
                    .get_upload_url(
                        &self.file.id,
                        &self.upload_state.upload_id,
                        self.upload_state.chunk_count,
                    )
                    .await
                {
                    let upload_urls: Vec<_> =
                        part_info_list.into_iter().map(|x| x.upload_url).collect();
                    self.upload_state.upload_urls = upload_urls;
                    upload_url = &self.upload_state.upload_urls[current_chunk as usize - 1];
                }
            }
            self.fs
                .drive
                .upload(upload_url, chunk_data.freeze())
                .await
                .map_err(|err| {
                    error!(
                        file_id = %self.file.id,
                        file_name = %self.file.name,
                        size = self.file.size,
                        error = %err,
                        "upload file chunk {} failed",
                        current_chunk
                    );
                    FsError::GeneralFailure
                })?;
            self.upload_state.chunk += 1;
        }
        Ok(())
    }
}

impl DavFile for AliyunDavFile {
    fn metadata(&'_ mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        debug!(file_id = %self.file.id, file_name = %self.file.name, "file: metadata");
        async move {
            let file = self.file.clone();
            Ok(Box::new(file) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn write_buf(&'_ mut self, buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        debug!(file_id = %self.file.id, file_name = %self.file.name, "file: write_buf");
        async move {
            self.prepare_for_upload().await?;
            self.upload_state.buffer.put(buf);
            self.maybe_upload_chunk(false).await?;
            Ok(())
        }
        .boxed()
    }

    fn write_bytes(&mut self, buf: Bytes) -> FsFuture<()> {
        debug!(file_id = %self.file.id, file_name = %self.file.name, "file: write_bytes");
        async move {
            self.prepare_for_upload().await?;
            self.upload_state.buffer.extend_from_slice(&buf);
            self.maybe_upload_chunk(false).await?;
            Ok(())
        }
        .boxed()
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<Bytes> {
        debug!(
            file_id = %self.file.id,
            file_name = %self.file.name,
            pos = self.current_pos,
            count = count,
            size = self.file.size,
            "file: read_bytes",
        );
        async move {
            if self.file.id.is_empty() {
                // upload in progress
                return Err(FsError::NotFound);
            }
            let download_url = self.download_url.take();
            let download_url = if let Some(mut url) = download_url {
                if is_url_expired(&url) {
                    debug!(url = %url, "download url expired");
                    url = self.get_download_url().await?;
                }
                url
            } else {
                self.get_download_url().await?
            };

            let content = self
                .fs
                .drive
                .download(&download_url, self.current_pos, count)
                .await
                .map_err(|err| {
                    error!(url = %download_url, error = %err, "download file failed");
                    FsError::NotFound
                })?;
            self.current_pos += content.len() as u64;
            self.download_url = Some(download_url);
            Ok(content)
        }
        .boxed()
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<u64> {
        debug!(
            file_id = %self.file.id,
            file_name = %self.file.name,
            pos = ?pos,
            "file: seek"
        );
        async move {
            let new_pos = match pos {
                SeekFrom::Start(pos) => pos,
                SeekFrom::End(pos) => (self.file.size as i64 - pos) as u64,
                SeekFrom::Current(size) => self.current_pos + size as u64,
            };
            self.current_pos = new_pos;
            Ok(new_pos)
        }
        .boxed()
    }

    fn flush(&mut self) -> FsFuture<()> {
        debug!(file_id = %self.file.id, file_name = %self.file.name, "file: flush");
        async move {
            self.prepare_for_upload().await?;
            self.maybe_upload_chunk(true).await?;
            if !self.upload_state.upload_id.is_empty() {
                self.fs
                    .drive
                    .complete_file_upload(&self.file.id, &self.upload_state.upload_id)
                    .await
                    .map_err(|err| {
                        error!(
                            file_id = %self.file.id,
                            file_name = %self.file.name,
                            error = %err,
                            "complete file upload failed"
                        );
                        FsError::GeneralFailure
                    })?;
            }
            self.fs
                .remove_uploading_file(&self.parent_file_id, &self.file.name);
            self.fs.dir_cache.invalidate_all();
            Ok(())
        }
        .boxed()
    }
}

fn is_url_expired(url: &str) -> bool {
    if let Ok(oss_url) = ::url::Url::parse(url) {
        let expires = oss_url.query_pairs().find_map(|(k, v)| {
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
            // 预留 1s
            return current_ts >= expires - 1;
        }
    }
    false
}
