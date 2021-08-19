use futures::future::FutureExt;
use log::debug;
use webdav_handler::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
        ReadDirMeta,
    },
};

use crate::aliyundrive::{AliyunDrive, AliyunFile};

#[derive(Debug, Clone)]
pub struct AliyunDriveFileSystem {
    drive: AliyunDrive,
}

impl AliyunDriveFileSystem {
    pub async fn new(refresh_token: String) -> Self {
        let drive = AliyunDrive::new(refresh_token).await;
        Self { drive }
    }
}

impl DavFileSystem for AliyunDriveFileSystem {
    fn open<'a>(&'a self, path: &'a DavPath, options: OpenOptions) -> FsFuture<Box<dyn DavFile>> {
        debug!("fs: open {}", path);
        todo!()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        meta: ReadDirMeta,
    ) -> FsFuture<FsStream<Box<dyn DavDirEntry>>> {
        debug!("fs: read_dir {}", path);
        let path = path.to_string();
        // FIXME: get parent_file_id by path
        let parent_file_id = "root";
        async move {
            let res = self
                .drive
                .list(parent_file_id)
                .await
                .map_err(|_| FsError::NotFound)?;
            let mut v: Vec<Box<dyn DavDirEntry>> = Vec::with_capacity(res.items.len());
            for file in res.items {
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
            if path.as_rel_ospath().parent().is_none() {
                let now = ::time::OffsetDateTime::now_utc().format(time::Format::Rfc3339);
                let root = AliyunFile {
                    drive_id: self.drive.drive_id.clone().unwrap(),
                    name: "/".to_string(),
                    id: "root".to_string(),
                    r#type: "folder".to_string(),
                    created_at: now.clone(),
                    updated_at: now,
                    size: 0,
                };
                return Ok(Box::new(root) as Box<dyn DavMetaData>);
            }
            todo!()
        }
        .boxed()
    }
}
