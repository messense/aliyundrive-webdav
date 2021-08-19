use std::{env, io};

use actix_web::{web, App, HttpServer};
use log::info;
use structopt::StructOpt;
use webdav_handler::actix::*;
use webdav_handler::{fakels::FakeLs, DavConfig, DavHandler};

use vfs::AliyunDriveFileSystem;

mod aliyundrive;
mod vfs;

pub async fn dav_handler(req: DavRequest, davhandler: web::Data<DavHandler>) -> DavResponse {
    if let Some(prefix) = req.prefix() {
        let config = DavConfig::new().strip_prefix(prefix);
        davhandler.handle_with(config, req.request).await.into()
    } else {
        davhandler.handle(req.request).await.into()
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "aliyundrive-webdav")]
struct Opt {
    /// Listen host
    #[structopt(long, default_value = "127.0.0.1")]
    host: String,
    /// Listen port
    #[structopt(short, long, default_value = "8080")]
    port: u16,
    /// Aliyun drive refresh token
    #[structopt(short, long, env = "REFRESH_TOKEN")]
    refresh_token: String,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "aliyundrive_webdav=info");
    }
    pretty_env_logger::init();

    let opt = Opt::from_args();

    let fs = AliyunDriveFileSystem::new(opt.refresh_token)
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "initialize aliyundrive file system failed",
            )
        })?;
    let dav_server = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(FakeLs::new())
        .build_handler();

    info!("listening on {}:{}", opt.host, opt.port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(dav_server.clone()))
            .service(web::resource("/{tail:.*}").to(dav_handler))
    })
    .bind((opt.host, opt.port))?
    .run()
    .await
}
