use std::convert::Infallible;
use std::net::ToSocketAddrs;
use std::{env, io, path::PathBuf};

use headers::{authorization::Basic, Authorization, HeaderMapExt};
use structopt::StructOpt;
use tracing::{debug, error, info};
use webdav_handler::{body::Body, memls::MemLs, DavConfig, DavHandler};

use drive::{AliyunDrive, DriveConfig};
use vfs::AliyunDriveFileSystem;

mod cache;
mod drive;
mod vfs;

#[derive(StructOpt, Debug)]
#[structopt(name = "aliyundrive-webdav")]
struct Opt {
    /// Listen host
    #[structopt(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,
    /// Listen port
    #[structopt(short, env = "PORT", long, default_value = "8080")]
    port: u16,
    /// Aliyun drive refresh token
    #[structopt(short, long, env = "REFRESH_TOKEN")]
    refresh_token: String,
    /// WebDAV authentication username
    #[structopt(short = "U", long, env = "WEBDAV_AUTH_USER")]
    auth_user: Option<String>,
    /// WebDAV authentication password
    #[structopt(short = "W", long, env = "WEBDAV_AUTH_PASSWORD")]
    auth_password: Option<String>,
    /// Automatically generate index.html
    #[structopt(short = "I", long)]
    auto_index: bool,
    /// Read/download buffer size in bytes, defaults to 10MB
    #[structopt(short = "S", long, default_value = "10485760")]
    read_buffer_size: usize,
    /// Directory entries cache size
    #[structopt(long, default_value = "1000")]
    cache_size: usize,
    /// Directory entries cache expiration time in seconds
    #[structopt(long, default_value = "600")]
    cache_ttl: u64,
    /// Root directory path
    #[structopt(long, default_value = "/")]
    root: String,
    /// Working directory, refresh_token will be stored in there if specified
    #[structopt(short = "w", long)]
    workdir: Option<PathBuf>,
    /// Delete file permanently instead of trashing it
    #[structopt(long, conflicts_with = "domain-id")]
    no_trash: bool,
    /// Aliyun PDS domain id
    #[structopt(long)]
    domain_id: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "native-tls-vendored")]
    openssl_probe::init_ssl_cert_env_vars();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "aliyundrive_webdav=info");
    }
    tracing_subscriber::fmt::init();

    let opt = Opt::from_args();
    let auth_user = opt.auth_user;
    let auth_pwd = opt.auth_password;
    if (auth_user.is_some() && auth_pwd.is_none()) || (auth_user.is_none() && auth_pwd.is_some()) {
        anyhow::bail!("auth-user and auth-password should be specified together.");
    }

    let (drive_config, no_trash) = if let Some(domain_id) = opt.domain_id {
        (
            DriveConfig {
                api_base_url: format!("https://{}.api.aliyunpds.com", domain_id),
                refresh_token_url: format!(
                    "https://{}.auth.aliyunpds.com/v2/account/token",
                    domain_id
                ),
                workdir: opt.workdir,
                app_id: Some("BasicUI".to_string()),
            },
            true, // PDS doesn't have trash support
        )
    } else {
        (
            DriveConfig {
                api_base_url: "https://api.aliyundrive.com".to_string(),
                refresh_token_url: "https://websv.aliyundrive.com/token/refresh".to_string(),
                workdir: opt.workdir,
                app_id: None,
            },
            opt.no_trash,
        )
    };
    let drive = AliyunDrive::new(drive_config, opt.refresh_token)
        .await
        .map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "initialize aliyundrive client failed")
        })?;
    let fs = AliyunDriveFileSystem::new(drive, opt.root, opt.cache_size, opt.cache_ttl, no_trash)
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
        .locksystem(MemLs::new())
        .read_buf_size(opt.read_buffer_size)
        .autoindex(opt.auto_index)
        .build_handler();
    debug!(
        read_buffer_size = opt.read_buffer_size,
        auto_index = opt.auto_index,
        "webdav handler initialized"
    );

    let addr = (opt.host, opt.port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    info!("listening on {:?}", addr);

    let make_service = hyper::service::make_service_fn(move |_| {
        let auth_user = auth_user.clone();
        let auth_pwd = auth_pwd.clone();
        let should_auth = auth_user.is_some() && auth_pwd.is_some();
        let dav_server = dav_server.clone();
        async move {
            let func = move |req: hyper::Request<hyper::Body>| {
                let dav_server = dav_server.clone();
                let auth_user = auth_user.clone();
                let auth_pwd = auth_pwd.clone();
                async move {
                    if should_auth {
                        let auth_user = auth_user.unwrap();
                        let auth_pwd = auth_pwd.unwrap();
                        let user = match req.headers().typed_get::<Authorization<Basic>>() {
                            Some(Authorization(basic))
                                if basic.username() == auth_user
                                    && basic.password() == auth_pwd =>
                            {
                                basic.username().to_string()
                            }
                            Some(_) | None => {
                                // return a 401 reply.
                                let response = hyper::Response::builder()
                                    .status(401)
                                    .header(
                                        "WWW-Authenticate",
                                        "Basic realm=\"aliyundrive-webdav\"",
                                    )
                                    .body(Body::from("Authentication required".to_string()))
                                    .unwrap();
                                return Ok(response);
                            }
                        };
                        let config = DavConfig::new().principal(user);
                        Ok::<_, Infallible>(dav_server.handle_with(config, req).await)
                    } else {
                        Ok::<_, Infallible>(dav_server.handle(req).await)
                    }
                }
            };
            Ok::<_, Infallible>(hyper::service::service_fn(func))
        }
    });

    let _ = hyper::Server::bind(&addr)
        .serve(make_service)
        .await
        .map_err(|e| error!("server error: {}", e));
    Ok(())
}
