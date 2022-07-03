use std::future::Future;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{env, io};

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use dav_server::{body::Body, memls::MemLs, DavConfig, DavHandler};
use headers::{authorization::Basic, Authorization, HeaderMapExt};
use hyper::{service::Service, Request, Response};
use self_update::cargo_crate_version;
use tracing::{debug, error, info, warn};

#[cfg(feature = "rustls-tls")]
use {
    futures_util::stream::StreamExt,
    hyper::server::accept,
    hyper::server::conn::AddrIncoming,
    std::fs::File,
    std::future::ready,
    std::path::Path,
    std::sync::Arc,
    tls_listener::{SpawningHandshakes, TlsListener},
    tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig},
    tokio_rustls::TlsAcceptor,
};

use drive::{parse_refresh_token, read_refresh_token, AliyunDrive, ClientType, DriveConfig};
use vfs::AliyunDriveFileSystem;

mod cache;
mod drive;
mod login;
mod vfs;

#[derive(Parser, Debug)]
#[clap(name = "aliyundrive-webdav", about, version, author)]
#[clap(args_conflicts_with_subcommands = true)]
struct Opt {
    /// Listen host
    #[clap(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,
    /// Listen port
    #[clap(short, env = "PORT", long, default_value = "8080")]
    port: u16,
    /// Aliyun drive refresh token
    #[clap(short, long, env = "REFRESH_TOKEN")]
    refresh_token: Option<String>,
    /// WebDAV authentication username
    #[clap(short = 'U', long, env = "WEBDAV_AUTH_USER")]
    auth_user: Option<String>,
    /// WebDAV authentication password
    #[clap(short = 'W', long, env = "WEBDAV_AUTH_PASSWORD")]
    auth_password: Option<String>,
    /// Automatically generate index.html
    #[clap(short = 'I', long)]
    auto_index: bool,
    /// Read/download buffer size in bytes, defaults to 10MB
    #[clap(short = 'S', long, default_value = "10485760")]
    read_buffer_size: usize,
    /// Directory entries cache size
    #[clap(long, default_value = "1000")]
    cache_size: u64,
    /// Directory entries cache expiration time in seconds
    #[clap(long, default_value = "600")]
    cache_ttl: u64,
    /// Root directory path
    #[clap(long, default_value = "/")]
    root: String,
    /// Working directory, refresh_token will be stored in there if specified
    #[clap(short = 'w', long)]
    workdir: Option<PathBuf>,
    /// Delete file permanently instead of trashing it
    #[clap(long, conflicts_with = "domain_id")]
    no_trash: bool,
    /// Aliyun PDS domain id
    #[clap(long)]
    domain_id: Option<String>,
    /// Enable read only mode
    #[clap(long)]
    read_only: bool,
    /// TLS certificate file path
    #[cfg(feature = "rustls-tls")]
    #[clap(long, env = "TLS_CERT")]
    tls_cert: Option<PathBuf>,
    /// TLS private key file path
    #[cfg(feature = "rustls-tls")]
    #[clap(long, env = "TLS_KEY")]
    tls_key: Option<PathBuf>,
    /// Prefix to be stripped off when handling request.
    #[clap(long, env = "WEBDAV_STRIP_PREFIX")]
    strip_prefix: Option<String>,
    /// Enable debug log
    #[clap(long)]
    debug: bool,
    /// Disable self auto upgrade
    #[clap(long)]
    no_self_upgrade: bool,

    #[clap(subcommand)]
    subcommands: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan QRCode
    #[clap(subcommand)]
    Qr(QrCommand),
}

#[derive(Subcommand, Debug)]
enum QrCommand {
    /// Scan QRCode login to get a token
    Login,
    /// Generate a QRCode
    Generate,
    /// Query the QRCode login result
    #[clap(arg_required_else_help = true)]
    Query {
        /// Query parameter t
        #[clap(long)]
        t: i64,
        /// Query parameter ck
        #[clap(long)]
        ck: String,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "native-tls-vendored")]
    openssl_probe::init_ssl_cert_env_vars();

    let opt = Opt::parse();
    if env::var("RUST_LOG").is_err() {
        if opt.debug {
            env::set_var("RUST_LOG", "aliyundrive_webdav=debug");
        } else {
            env::set_var("RUST_LOG", "aliyundrive_webdav=info");
        }
    }
    tracing_subscriber::fmt::init();

    // subcommands
    match opt.subcommands.as_ref() {
        Some(Commands::Qr(qr)) => {
            match qr {
                QrCommand::Login => {
                    let refresh_token = login(120).await?;
                    println!("refresh_token: {}", refresh_token)
                }
                QrCommand::Generate => {
                    let scan = login::QrCodeScanner::new().await?;
                    let result = scan.generator().await?;
                    let data = result.get_content_data().context("Failed to get QRCode")?;
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
                QrCommand::Query { t, ck } => {
                    use crate::login::model::{AuthorizationToken, QueryQrCodeCkForm};

                    let scan = login::QrCodeScanner::new().await?;
                    let form = QueryQrCodeCkForm::new(*t, ck.to_string());
                    let query_result = scan.query(&form).await?;
                    if query_result.is_confirmed() {
                        let refresh_token = query_result
                            .get_mobile_login_result()
                            .context("failed to get mobile login result")?
                            .refresh_token()
                            .context("failed to get refresh token")?;
                        println!("{}", refresh_token)
                    }
                }
            }
            return Ok(());
        }
        None => {}
    }

    if env::var("NO_SELF_UPGRADE").is_err() && !opt.no_self_upgrade {
        tokio::task::spawn_blocking(move || {
            if let Err(e) = check_for_update(opt.debug) {
                warn!("failed to check for update: {}", e);
            }
        })
        .await?;
    }

    let auth_user = opt.auth_user;
    let auth_password = opt.auth_password;
    if (auth_user.is_some() && auth_password.is_none())
        || (auth_user.is_none() && auth_password.is_some())
    {
        anyhow::bail!("auth-user and auth-password must be specified together.");
    }

    #[cfg(feature = "rustls-tls")]
    let use_tls = match (opt.tls_cert.as_ref(), opt.tls_key.as_ref()) {
        (Some(_), Some(_)) => true,
        (None, None) => false,
        _ => anyhow::bail!("tls-cert and tls-key must be specified together."),
    };

    let workdir = opt
        .workdir
        .or_else(|| dirs::cache_dir().map(|c| c.join("aliyundrive-webdav")));
    let refresh_token_from_file = if let Some(dir) = workdir.as_ref() {
        read_refresh_token(dir).await.ok()
    } else {
        None
    };
    let (refresh_token, client_type) = if opt.domain_id.is_none()
        && opt.refresh_token.is_none()
        && refresh_token_from_file.is_none()
        && atty::is(atty::Stream::Stdout)
    {
        (login(30).await?, ClientType::App)
    } else {
        parse_refresh_token(&opt.refresh_token.unwrap_or_default())?
    };
    let (drive_config, no_trash) = if let Some(domain_id) = opt.domain_id.as_ref() {
        (
            DriveConfig {
                api_base_url: format!("https://{}.api.aliyunpds.com", domain_id),
                refresh_token_url: format!(
                    "https://{}.auth.aliyunpds.com/v2/account/token",
                    domain_id
                ),
                workdir,
                app_id: Some("BasicUI".to_string()),
                client_type: ClientType::Web,
            },
            true, // PDS doesn't have trash support
        )
    } else {
        (
            DriveConfig {
                api_base_url: "https://api.aliyundrive.com".to_string(),
                refresh_token_url: String::new(),
                workdir,
                app_id: None,
                client_type,
            },
            opt.no_trash,
        )
    };

    let drive = AliyunDrive::new(drive_config, refresh_token).await?;
    let fs = AliyunDriveFileSystem::new(
        drive,
        opt.root,
        opt.cache_size,
        opt.cache_ttl,
        no_trash,
        opt.read_only,
    )
    .await?;
    debug!("aliyundrive file system initialized");

    let mut dav_server_builder = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(MemLs::new())
        .read_buf_size(opt.read_buffer_size)
        .autoindex(opt.auto_index);
    if let Some(prefix) = opt.strip_prefix {
        dav_server_builder = dav_server_builder.strip_prefix(prefix);
    }

    let dav_server = dav_server_builder.build_handler();
    debug!(
        read_buffer_size = opt.read_buffer_size,
        auto_index = opt.auto_index,
        "webdav handler initialized"
    );

    let addr = (opt.host, opt.port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::AddrNotAvailable))?;

    #[cfg(feature = "rustls-tls")]
    if use_tls {
        let tls_key = opt.tls_key.as_ref().unwrap();
        let tls_cert = opt.tls_cert.as_ref().unwrap();
        let incoming = TlsListener::new(
            SpawningHandshakes(tls_acceptor(tls_key, tls_cert)?),
            AddrIncoming::bind(&addr)?,
        )
        .filter(|conn| {
            if let Err(err) = conn {
                error!("TLS error: {:?}", err);
                ready(false)
            } else {
                ready(true)
            }
        });
        let server = hyper::Server::builder(accept::from_stream(incoming)).serve(MakeSvc {
            auth_user: auth_user.clone(),
            auth_password: auth_password.clone(),
            handler: dav_server.clone(),
        });
        info!("listening on https://{}", addr);
        let _ = server.await.map_err(|e| error!("server error: {}", e));
        return Ok(());
    }
    let server = hyper::Server::bind(&addr).serve(MakeSvc {
        auth_user,
        auth_password,
        handler: dav_server,
    });
    info!("listening on http://{}", server.local_addr());
    let _ = server.await.map_err(|e| error!("server error: {}", e));
    Ok(())
}

#[derive(Clone)]
struct AliyunDriveWebDav {
    auth_user: Option<String>,
    auth_password: Option<String>,
    handler: DavHandler,
}

impl Service<Request<hyper::Body>> for AliyunDriveWebDav {
    type Response = Response<Body>;
    type Error = hyper::Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<hyper::Body>) -> Self::Future {
        let should_auth = self.auth_user.is_some() && self.auth_password.is_some();
        let dav_server = self.handler.clone();
        let auth_user = self.auth_user.clone();
        let auth_pwd = self.auth_password.clone();
        Box::pin(async move {
            if should_auth {
                let auth_user = auth_user.unwrap();
                let auth_pwd = auth_pwd.unwrap();
                let user = match req.headers().typed_get::<Authorization<Basic>>() {
                    Some(Authorization(basic))
                        if basic.username() == auth_user && basic.password() == auth_pwd =>
                    {
                        basic.username().to_string()
                    }
                    Some(_) | None => {
                        // return a 401 reply.
                        let response = hyper::Response::builder()
                            .status(401)
                            .header("WWW-Authenticate", "Basic realm=\"aliyundrive-webdav\"")
                            .body(Body::from("Authentication required".to_string()))
                            .unwrap();
                        return Ok(response);
                    }
                };
                let config = DavConfig::new().principal(user);
                Ok(dav_server.handle_with(config, req).await)
            } else {
                Ok(dav_server.handle(req).await)
            }
        })
    }
}

struct MakeSvc {
    auth_user: Option<String>,
    auth_password: Option<String>,
    handler: DavHandler,
}

impl<T> Service<T> for MakeSvc {
    type Response = AliyunDriveWebDav;
    type Error = hyper::Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let auth_user = self.auth_user.clone();
        let auth_password = self.auth_password.clone();
        let handler = self.handler.clone();
        let fut = async move {
            Ok(AliyunDriveWebDav {
                auth_user,
                auth_password,
                handler,
            })
        };
        Box::pin(fut)
    }
}

#[cfg(feature = "rustls-tls")]
fn tls_acceptor(key: &Path, cert: &Path) -> anyhow::Result<TlsAcceptor> {
    let mut key_reader = io::BufReader::new(File::open(key)?);
    let mut cert_reader = io::BufReader::new(File::open(cert)?);

    let key = PrivateKey(private_keys(&mut key_reader)?.remove(0));
    let certs = rustls_pemfile::certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();

    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config).into())
}

#[cfg(feature = "rustls-tls")]
fn private_keys(rd: &mut dyn io::BufRead) -> Result<Vec<Vec<u8>>, io::Error> {
    use rustls_pemfile::{read_one, Item};

    let mut keys = Vec::<Vec<u8>>::new();
    loop {
        match read_one(rd)? {
            None => return Ok(keys),
            Some(Item::RSAKey(key)) => keys.push(key),
            Some(Item::PKCS8Key(key)) => keys.push(key),
            Some(Item::ECKey(key)) => keys.push(key),
            _ => {}
        };
    }
}

async fn login(timeout: u64) -> anyhow::Result<String> {
    use crate::login::model::{AuthorizationToken, Ok, QueryQrCodeCkForm};

    const SLEEP: u64 = 3;

    let scan = login::QrCodeScanner::new().await?;
    // 返回二维码内容结果集
    let generator_qr_code_result = scan.generator().await?;
    // 需要生成二维码的内容
    let qrcode_content = generator_qr_code_result.get_content();
    let ck_form = QueryQrCodeCkForm::from(generator_qr_code_result);
    // 打印二维码
    qr2term::print_qr(&qrcode_content)?;
    info!("Please scan the qrcode to login in {} seconds", timeout);
    let loop_count = timeout / SLEEP;
    for _i in 0..loop_count {
        tokio::time::sleep(tokio::time::Duration::from_secs(SLEEP)).await;
        // 模拟轮训查询二维码状态
        let query_result = scan.query(&ck_form).await?;
        if query_result.ok() {
            // query_result.is_new() 表示未扫码状态
            if query_result.is_new() {
                // 做点什么..
                continue;
            }
            // query_result.is_expired() 表示扫码成功，但未点击确认登陆
            if query_result.is_expired() {
                // 做点什么..
                debug!("login expired");
                continue;
            }
            // 移动端APP扫码成功并确认登陆
            if query_result.is_confirmed() {
                // 获取移动端登陆Result
                let mobile_login_result = query_result
                    .get_mobile_login_result()
                    .context("failed to get mobile login result")?;
                // 移动端 refresh token
                let refresh_token = mobile_login_result
                    .refresh_token()
                    .context("failed to get refresh token")?;
                return Ok(refresh_token);
            }
        }
    }
    anyhow::bail!("Login failed")
}

fn check_for_update(show_output: bool) -> anyhow::Result<()> {
    use self_update::update::UpdateStatus;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let auth_token = env::var("GITHUB_TOKEN")
        .unwrap_or_else(|_| env::var("HOMEBREW_GITHUB_API_TOKEN").unwrap_or_default());
    let status = self_update::backends::github::Update::configure()
        .repo_owner("messense")
        .repo_name("aliyundrive-webdav")
        .bin_name("aliyundrive-webdav")
        .target(if cfg!(target_os = "macos") {
            "apple-darwin"
        } else {
            self_update::get_target()
        })
        .auth_token(&auth_token)
        .show_output(show_output)
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update_extended()?;
    if let UpdateStatus::Updated(ref release) = status {
        if let Some(body) = &release.body {
            if !body.trim().is_empty() {
                info!("aliyundrive-webdav upgraded to {}:\n", release.version);
                info!("{}", body);
            } else {
                info!("aliyundrive-webdav upgraded to {}", release.version);
            }
        }
    } else {
        info!("aliyundrive-webdav is up-to-date");
    }

    if status.updated() {
        warn!("Respawning...");
        let current_exe = env::current_exe();
        let mut command = Command::new(current_exe?);
        command.args(env::args().skip(1)).env("NO_SELF_UPGRADE", "");
        #[cfg(unix)]
        {
            let err = command.exec();
            anyhow::bail!(err);
        }

        #[cfg(windows)]
        {
            let status = command.spawn().and_then(|mut c| c.wait())?;
            anyhow::bail!("aliyundrive-webdav upgraded");
        }
    }
    Ok(())
}
