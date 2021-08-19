use futures::future::FutureExt;
use log::{debug, trace};
use moka::future::Cache;
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
    pub async fn new(refresh_token: String) -> Self {
        let drive = AliyunDrive::new(refresh_token).await;
        let file_ids = Cache::new(100000000);
        Self { drive, file_ids }
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
        todo!()
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
