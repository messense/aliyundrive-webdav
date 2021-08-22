use std::io::SeekFrom;
use std::time::Duration;

use anyhow::Result;
use async_recursion::async_recursion;
use bytes::{Buf, Bytes};
use futures_util::future::{self, FutureExt};
use moka::future::{Cache, CacheBuilder};
use tracing::{debug, trace};
use webdav_handler::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
        ReadDirMeta,
    },
};

use crate::drive::{AliyunDrive, AliyunFile};

#[derive(Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
struct ENCODE_SET;

impl percent_encoding::EncodeSet for ENCODE_SET {
    // Encode all non-unreserved characters, except '/'.
    // See RFC3986, and https://en.wikipedia.org/wiki/Percent-encoding .
    #[inline]
    fn contains(&self, byte: u8) -> bool {
        let unreserved = (b'A'..=b'Z').contains(&byte)
            || (b'a'..=b'z').contains(&byte)
            || (b'0'..=b'9').contains(&byte)
            || byte == b'-'
            || byte == b'_'
            || byte == b'.'
            || byte == b'~';
        !unreserved && byte != b'/'
    }
}

// encode path segment with user-defined ENCODE_SET
fn encode_path(src: &[u8]) -> String {
    percent_encoding::percent_encode(src, ENCODE_SET).to_string()
}

#[derive(Clone)]
pub struct AliyunDriveFileSystem {
    drive: AliyunDrive,
    file_ids: Cache<String, String>,
    read_dir_cache: Cache<String, Vec<AliyunFile>>,
    file_cache: Cache<String, AliyunFile>,
}

impl AliyunDriveFileSystem {
    pub async fn new(refresh_token: String) -> Result<Self> {
        let drive = AliyunDrive::new(refresh_token).await?;
        let file_ids = CacheBuilder::new(100000)
            .initial_capacity(100)
            .time_to_live(Duration::from_secs(30 * 60))
            .build();
        debug!("file id cache initialized");
        let read_dir_cache = CacheBuilder::new(100)
            .time_to_live(Duration::from_secs(10 * 60))
            .build();
        debug!("read_dir cache initialized");
        let file_cache = CacheBuilder::new(10000)
            .time_to_live(Duration::from_secs(60 * 60))
            .build();
        debug!("file cache initialized");
        Ok(Self {
            drive,
            file_ids,
            read_dir_cache,
            file_cache,
        })
    }

    async fn get_file_id(&self, dav_path: &DavPath) -> Result<Option<String>, FsError> {
        let path = dav_path.as_rel_ospath();
        if path.parent().is_none() {
            Ok(Some("root".to_string()))
        } else {
            let path_str = path.to_string_lossy().into_owned();
            match self.file_ids.get(&path_str) {
                Some(file_id) => {
                    trace!("found {} file_id: {} in cache", path_str, file_id);
                    Ok(Some(file_id))
                }
                None => {
                    trace!("{} file_id not found", path_str);
                    let parts: Vec<&str> = path_str.split('/').collect();
                    let parts_len = parts.len();
                    let filename = parts[parts_len - 1];

                    // find in root first
                    let files = self.read_dir_and_cache(&DavPath::new("/").unwrap()).await?;
                    if let Some(file) = files.iter().find(|f| f.name == filename) {
                        trace!("found {} file_id: {}", path_str, file.id);
                        return Ok(Some(file.id.clone()));
                    }

                    let mut prefix = String::new();
                    for part in &parts[0..parts_len - 1] {
                        let parent = format!("{}/{}", prefix, part);
                        let parent_path = DavPath::new(&encode_path(parent.as_bytes()))
                            .map_err(|_| FsError::GeneralFailure)?;
                        prefix = parent;
                        let files = self.read_dir_and_cache(&parent_path).await?;
                        if let Some(file) = files.iter().find(|f| f.name == filename) {
                            trace!("found {} file_id: {}", path_str, file.id);
                            return Ok(Some(file.id.clone()));
                        }
                    }
                    Ok(None)
                }
            }
        }
    }

    #[async_recursion]
    async fn read_dir_and_cache(&self, path: &DavPath) -> Result<Vec<AliyunFile>, FsError> {
        debug!(path = %path.as_rel_ospath().display(), "read_dir and cache");
        let parent_file_id = self.get_file_id(path).await?.ok_or(FsError::NotFound)?;
        let files = if let Some(files) = self.read_dir_cache.get(&parent_file_id) {
            files
        } else {
            let items = self
                .drive
                .list_all(&parent_file_id)
                .await
                .map_err(|_| FsError::NotFound)?;
            self.cache_read_dir(path, parent_file_id, items.clone())
                .await;
            items
        };
        Ok(files)
    }

    async fn cache_file_id(&self, path: String, file_id: String) {
        trace!(path = %path, file_id = %file_id, "cache file_id");
        self.file_ids.insert(path, file_id).await;
    }

    async fn cache_file(&self, file_id: String, file: AliyunFile) {
        trace!(file_id = %file_id, file_name = %file.name, "cache file");
        self.file_cache.insert(file_id, file).await;
    }

    async fn cache_read_dir(&self, path: &DavPath, file_id: String, files: Vec<AliyunFile>) {
        trace!(file_id = %file_id, count = files.len(), "cache read_dir");
        let rel_path = path.as_rel_ospath();
        for file in &files {
            self.cache_file(file.id.clone(), file.clone()).await;
            let file_path = rel_path.join(&file.name).to_string_lossy().into_owned();
            self.cache_file_id(file_path, file.id.clone()).await;
        }
        self.read_dir_cache.insert(file_id, files).await;
    }

    async fn get_file(&self, file_id: String) -> Result<AliyunFile, FsError> {
        if let Some(file) = self.file_cache.get(&file_id) {
            Ok(file)
        } else {
            let file = self
                .drive
                .get(&file_id)
                .await
                .map_err(|_| FsError::NotFound)?;
            self.cache_file(file_id, file.clone()).await;
            Ok(file)
        }
    }
}

impl DavFileSystem for AliyunDriveFileSystem {
    fn open<'a>(&'a self, path: &'a DavPath, _options: OpenOptions) -> FsFuture<Box<dyn DavFile>> {
        debug!(path = %path.as_rel_ospath().display(), "fs: open");
        async move {
            let file_id = self.get_file_id(path).await?.ok_or(FsError::NotFound)?;
            let file = self.get_file(file_id).await?;
            let dav_file = AliyunDavFile::new(self.drive.clone(), file);
            Ok(Box::new(dav_file) as Box<dyn DavFile>)
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<FsStream<Box<dyn DavDirEntry>>> {
        debug!(path = %path.as_rel_ospath().display(), "fs: read_dir");
        async move {
            let files = self.read_dir_and_cache(path).await?;
            let mut v: Vec<Box<dyn DavDirEntry>> = Vec::with_capacity(files.len());
            let rel_path = path.as_rel_ospath();
            for file in files {
                let file_path = rel_path.join(&file.name).to_string_lossy().into_owned();
                self.cache_file_id(file_path, file.id.clone()).await;
                v.push(Box::new(file));
            }
            let stream = futures_util::stream::iter(v);
            Ok(Box::pin(stream) as FsStream<Box<dyn DavDirEntry>>)
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        debug!(path = %path.as_rel_ospath().display(), "fs: metadata");
        async move {
            let file_id = self.get_file_id(path).await?.ok_or(FsError::NotFound)?;
            if &file_id == "root" {
                let root = AliyunFile::new_root(self.drive.drive_id.clone().unwrap());
                Ok(Box::new(root) as Box<dyn DavMetaData>)
            } else {
                let file = self.get_file(file_id).await?;
                Ok(Box::new(file) as Box<dyn DavMetaData>)
            }
        }
        .boxed()
    }
}

#[derive(Debug, Clone)]
struct AliyunDavFile {
    drive: AliyunDrive,
    file: AliyunFile,
    current_pos: u64,
}

impl AliyunDavFile {
    fn new(drive: AliyunDrive, file: AliyunFile) -> Self {
        Self {
            drive,
            file,
            current_pos: 0,
        }
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

    fn write_buf(&'_ mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        Box::pin(future::ready(Err(FsError::NotImplemented)))
    }

    fn write_bytes(&mut self, _buf: Bytes) -> FsFuture<()> {
        Box::pin(future::ready(Err(FsError::NotImplemented)))
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
            let download_url = self.file.download_url.as_ref().ok_or(FsError::NotFound)?;
            let content = self
                .drive
                .download(&self.file.id, download_url, self.current_pos, count)
                .await
                .map_err(|_| FsError::NotFound)?;
            self.current_pos += content.len() as u64;
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
        Box::pin(future::ready(Err(FsError::NotImplemented)))
    }
}
