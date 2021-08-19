use std::io::SeekFrom;
use std::time::Duration;

use anyhow::Result;
use bytes::{Buf, Bytes};
use futures::future::FutureExt;
use log::{debug, trace};
use moka::future::{Cache, CacheBuilder};
use webdav_handler::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
        ReadDirMeta,
    },
};

use crate::aliyundrive::{AliyunDrive, AliyunFile};

#[derive(Clone)]
pub struct AliyunDriveFileSystem {
    drive: AliyunDrive,
    file_ids: Cache<String, String>,
}

impl AliyunDriveFileSystem {
    pub async fn new(refresh_token: String) -> Result<Self> {
        let drive = AliyunDrive::new(refresh_token).await?;
        let file_ids = CacheBuilder::new(100000)
            .initial_capacity(100)
            .time_to_live(Duration::from_secs(30 * 60))
            .time_to_idle(Duration::from_secs(5 * 60))
            .build();
        debug!("file id cache initialized");
        Ok(Self { drive, file_ids })
    }

    async fn get_file_id(&self, path: &DavPath) -> Option<String> {
        let path = path.as_rel_ospath();
        if path.parent().is_none() {
            Some("root".to_string())
        } else {
            let path_str = path.to_string_lossy().into_owned();
            match self.file_ids.get(&path_str) {
                Some(file_id) => {
                    trace!("found {} file_id: {}", path_str, file_id);
                    Some(file_id)
                }
                None => {
                    trace!("{} file_id not found", path_str);
                    None
                }
            }
        }
    }

    async fn cache_file_id(&self, path: String, file_id: String) {
        trace!("cache {} file_id: {}", path, file_id);
        self.file_ids.insert(path, file_id).await;
    }
}

impl DavFileSystem for AliyunDriveFileSystem {
    fn open<'a>(&'a self, path: &'a DavPath, _options: OpenOptions) -> FsFuture<Box<dyn DavFile>> {
        debug!("fs: open {}", path);
        async move {
            let file_id = self.get_file_id(path).await.ok_or(FsError::NotFound)?;
            let file = self
                .drive
                .get(&file_id)
                .await
                .map_err(|_| FsError::NotFound)?;
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
        debug!("fs: read_dir {}", path);
        async move {
            let parent_file_id = self.get_file_id(path).await.ok_or(FsError::NotFound)?;
            let files = self
                .drive
                .list_all(&parent_file_id)
                .await
                .map_err(|_| FsError::NotFound)?;
            let mut v: Vec<Box<dyn DavDirEntry>> = Vec::with_capacity(files.len());
            let rel_path = path.as_rel_ospath();
            for file in files {
                let file_path = rel_path.join(&file.name).to_string_lossy().into_owned();
                self.cache_file_id(file_path, file.id.clone()).await;
                v.push(Box::new(file));
            }
            let stream = futures::stream::iter(v);
            Ok(Box::pin(stream) as FsStream<Box<dyn DavDirEntry>>)
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        debug!("fs: metadata {}", path);
        async move {
            let file_id = self.get_file_id(path).await.ok_or(FsError::NotFound)?;
            if &file_id == "root" {
                let now = ::time::OffsetDateTime::now_utc().format(time::Format::Rfc3339);
                let root = AliyunFile {
                    drive_id: self.drive.drive_id.clone().unwrap(),
                    name: "/".to_string(),
                    id: file_id,
                    r#type: "folder".to_string(),
                    created_at: now.clone(),
                    updated_at: now,
                    size: 0,
                    download_url: None,
                };
                Ok(Box::new(root) as Box<dyn DavMetaData>)
            } else {
                let file = self
                    .drive
                    .get(&file_id)
                    .await
                    .map_err(|_| FsError::NotFound)?;
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
    fn metadata<'a>(&'a mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            let file = self.file.clone();
            Ok(Box::new(file) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn write_buf<'a>(&'a mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        todo!()
    }

    fn write_bytes<'a>(&'a mut self, _buf: Bytes) -> FsFuture<'_, ()> {
        todo!()
    }

    fn read_bytes<'a>(&'a mut self, count: usize) -> FsFuture<'_, Bytes> {
        async move {
            let download_url = self.file.download_url.as_ref().ok_or(FsError::NotFound)?;
            let content = self
                .drive
                .download(&download_url, self.current_pos, count)
                .await
                .map_err(|_| FsError::NotFound)?;
            self.current_pos += content.len() as u64;
            Ok(content)
        }
        .boxed()
    }

    fn seek<'a>(&'a mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        trace!("{} seek {:?}", self.file.id, pos);
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

    fn flush<'a>(&'a mut self) -> FsFuture<'_, ()> {
        todo!()
    }
}
