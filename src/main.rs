use std::convert::Infallible;
use std::net::ToSocketAddrs;
use std::{env, io};

use log::{debug, info};
use structopt::StructOpt;
use webdav_handler::{fakels::FakeLs, DavHandler};

use vfs::AliyunDriveFileSystem;

mod aliyundrive;
mod vfs;

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

#[tokio::main]
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
    debug!("aliyundrive file system initialized");

    let dav_server = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(FakeLs::new())
        .build_handler();
    debug!("webdav handler initialized");

    let addr = (opt.host, opt.port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    info!("listening on {:?}", addr);

    let make_service = hyper::service::make_service_fn(move |_| {
        let dav_server = dav_server.clone();
        async move {
            let func = move |req| {
                let dav_server = dav_server.clone();
                async move { Ok::<_, Infallible>(dav_server.handle(req).await) }
            };
            Ok::<_, Infallible>(hyper::service::service_fn(func))
        }
    });

    let _ = hyper::Server::bind(&addr)
        .serve(make_service)
        .await
        .map_err(|e| eprintln!("server error: {}", e));
    Ok(())
}
