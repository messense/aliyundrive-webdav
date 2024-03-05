use std::env;
use std::path::PathBuf;

use anyhow::bail;
use clap::{Parser, Subcommand};
use dav_server::{memls::MemLs, DavHandler};
#[cfg(unix)]
use futures_util::stream::StreamExt;
use self_update::cargo_crate_version;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
#[cfg(unix)]
use {signal_hook::consts::signal::*, signal_hook_tokio::Signals};

use cache::Cache;
use drive::{read_refresh_token, AliyunDrive, DriveConfig, DriveType};
use vfs::AliyunDriveFileSystem;
use webdav::WebDavServer;

mod cache;
mod drive;
mod login;
mod vfs;
mod webdav;

#[derive(Parser, Debug)]
#[command(name = "aliyundrive-webdav", about, version, author)]
#[command(args_conflicts_with_subcommands = true)]
struct Opt {
    /// Listen host
    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,
    /// Listen port
    #[arg(short, env = "PORT", long, default_value = "8080")]
    port: u16,
    /// Aliyun drive client_id
    #[arg(long, env = "CLIENT_ID")]
    client_id: Option<String>,
    /// Aliyun drive client_secret
    #[arg(long, env = "CLIENT_SECRET")]
    client_secret: Option<String>,
    /// Aliyun drive type
    #[arg(long, env = "DRIVE_TYPE")]
    drive_type: Option<DriveType>,
    /// Aliyun drive refresh token
    #[arg(short, long, env = "REFRESH_TOKEN")]
    refresh_token: Option<String>,
    /// WebDAV authentication username
    #[arg(short = 'U', long, env = "WEBDAV_AUTH_USER")]
    auth_user: Option<String>,
    /// WebDAV authentication password
    #[arg(short = 'W', long, env = "WEBDAV_AUTH_PASSWORD")]
    auth_password: Option<String>,
    /// Automatically generate index.html
    #[arg(short = 'I', long)]
    auto_index: bool,
    /// Read/download buffer size in bytes, defaults to 10MB
    #[arg(short = 'S', long, default_value = "10485760")]
    read_buffer_size: usize,
    /// Upload buffer size in bytes, defaults to 16MB
    #[arg(long, default_value = "16777216")]
    upload_buffer_size: usize,
    /// Directory entries cache size
    #[arg(long, default_value = "1000")]
    cache_size: u64,
    /// Directory entries cache expiration time in seconds
    #[arg(long, default_value = "600")]
    cache_ttl: u64,
    /// Root directory path
    #[arg(long, env = "WEBDAV_ROOT", default_value = "/")]
    root: String,
    /// Working directory, refresh_token will be stored in there if specified
    #[arg(short = 'w', long)]
    workdir: Option<PathBuf>,
    /// Delete file permanently instead of trashing it
    #[arg(long)]
    no_trash: bool,
    /// Enable read only mode
    #[arg(long)]
    read_only: bool,
    /// TLS certificate file path
    #[arg(long, env = "TLS_CERT")]
    tls_cert: Option<PathBuf>,
    /// TLS private key file path
    #[arg(long, env = "TLS_KEY")]
    tls_key: Option<PathBuf>,
    /// Prefix to be stripped off when handling request.
    #[arg(long, env = "WEBDAV_STRIP_PREFIX")]
    strip_prefix: Option<String>,
    /// Enable debug log
    #[arg(long)]
    debug: bool,
    /// Disable self auto upgrade
    #[arg(long)]
    no_self_upgrade: bool,
    /// Skip uploading same size file
    #[arg(long)]
    skip_upload_same_size: bool,
    /// Prefer downloading using HTTP protocol
    #[arg(long)]
    prefer_http_download: bool,
    /// Enable 302 redirect when possible
    #[arg(long)]
    redirect: bool,

    #[command(subcommand)]
    subcommands: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan QRCode
    #[command(subcommand)]
    Qr(QrCommand),
}

#[derive(Subcommand, Debug)]
enum QrCommand {
    /// Scan QRCode login to get a token
    Login,
    /// Generate a QRCode
    Generate,
    /// Query the QRCode login result
    #[command(arg_required_else_help = true)]
    Query {
        /// Query parameter sid
        #[arg(long)]
        sid: String,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "native-tls-vendored")]
    openssl_probe::init_ssl_cert_env_vars();

    let opt = Opt::parse();
    if env::var("RUST_LOG").is_err() {
        if opt.debug {
            env::set_var("RUST_LOG", "aliyundrive_webdav=debug,reqwest=debug");
        } else {
            env::set_var("RUST_LOG", "aliyundrive_webdav=info,reqwest=warn");
        }
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_timer(tracing_subscriber::fmt::time::time())
        .init();

    let workdir = opt
        .workdir
        .or_else(|| dirs::cache_dir().map(|c| c.join("aliyundrive-webdav")));
    let refresh_token_host = if opt.client_id.is_none() || opt.client_secret.is_none() {
        env::var("ALIYUNDRIVE_OAUTH_SERVER")
            .unwrap_or_else(|_| "https://aliyundrive-oauth.messense.me".to_string())
    } else {
        "https://openapi.aliyundrive.com".to_string()
    };
    let drive_config = DriveConfig {
        api_base_url: "https://openapi.aliyundrive.com".to_string(),
        refresh_token_host,
        workdir,
        client_id: opt.client_id.clone(),
        client_secret: opt.client_secret.clone(),
        drive_type: opt.drive_type.clone(),
    };

    // subcommands
    if let Some(Commands::Qr(qr)) = opt.subcommands.as_ref() {
        match qr {
            QrCommand::Login => {
                let refresh_token = login(drive_config.clone(), 120).await?;
                println!("\nrefresh_token:\n\n{}", refresh_token)
            }
            QrCommand::Generate => {
                let scanner = login::QrCodeScanner::new(drive_config.clone()).await?;
                let data = scanner.scan().await?;
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            QrCommand::Query { sid } => {
                let scanner = login::QrCodeScanner::new(drive_config.clone()).await?;
                let query_result = scanner.query(sid).await?;
                if query_result.is_success() {
                    let code = query_result.auth_code.unwrap();
                    let refresh_token = scanner.fetch_refresh_token(&code).await?;
                    println!("{}", refresh_token)
                }
            }
        }
        return Ok(());
    }

    if env::var("NO_SELF_UPGRADE").is_err() && !opt.no_self_upgrade {
        tokio::task::spawn_blocking(move || {
            if let Err(e) = check_for_update(opt.debug) {
                debug!("failed to check for update: {}", e);
            }
        })
        .await?;
    }

    let auth_user = opt.auth_user;
    let auth_password = opt.auth_password;
    if (auth_user.is_some() && auth_password.is_none())
        || (auth_user.is_none() && auth_password.is_some())
    {
        bail!("auth-user and auth-password must be specified together.");
    }

    let tls_config = match (opt.tls_cert, opt.tls_key) {
        (Some(cert), Some(key)) => Some((cert, key)),
        (None, None) => None,
        _ => bail!("tls-cert and tls-key must be specified together."),
    };

    let refresh_token_from_file = if let Some(dir) = drive_config.workdir.as_ref() {
        read_refresh_token(dir).await.ok()
    } else {
        None
    };
    let refresh_token = if opt.refresh_token.is_none()
        && refresh_token_from_file.is_none()
        && atty::is(atty::Stream::Stdout)
    {
        login(drive_config.clone(), 30).await?
    } else {
        let token = opt.refresh_token.unwrap_or_default();
        if !token.is_empty() && token.split('.').count() < 3 {
            bail!("Invalid refresh token value found in `--refresh-token` argument");
        }
        token
    };

    let drive = AliyunDrive::new(drive_config, refresh_token).await?;
    let mut fs = AliyunDriveFileSystem::new(drive, opt.root, opt.cache_size, opt.cache_ttl)?;
    fs.set_no_trash(opt.no_trash)
        .set_read_only(opt.read_only)
        .set_upload_buffer_size(opt.upload_buffer_size)
        .set_skip_upload_same_size(opt.skip_upload_same_size)
        .set_prefer_http_download(opt.prefer_http_download);
    debug!("aliyundrive file system initialized");

    #[cfg(unix)]
    let dir_cache = fs.dir_cache.clone();

    let mut dav_server_builder = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(MemLs::new())
        .read_buf_size(opt.read_buffer_size)
        .autoindex(opt.auto_index)
        .redirect(opt.redirect);
    if let Some(prefix) = opt.strip_prefix {
        dav_server_builder = dav_server_builder.strip_prefix(prefix);
    }

    let dav_server = dav_server_builder.build_handler();
    debug!(
        read_buffer_size = opt.read_buffer_size,
        auto_index = opt.auto_index,
        "webdav handler initialized"
    );

    let server = WebDavServer {
        host: opt.host,
        port: opt.port,
        auth_user,
        auth_password,
        tls_config,
        handler: dav_server,
    };

    #[cfg(not(unix))]
    server.serve().await?;

    #[cfg(unix)]
    {
        let signals = Signals::new([SIGHUP])?;
        let handle = signals.handle();
        let signals_task = tokio::spawn(handle_signals(signals, dir_cache));

        server.serve().await?;

        // Terminate the signal stream.
        handle.close();
        signals_task.await?;
    }
    Ok(())
}

#[cfg(unix)]
async fn handle_signals(mut signals: Signals, dir_cache: Cache) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGHUP => {
                dir_cache.invalidate_all();
                info!("directory cache invalidated by SIGHUP");
            }
            _ => unreachable!(),
        }
    }
}

async fn login(drive_config: DriveConfig, timeout: u64) -> anyhow::Result<String> {
    const SLEEP: u64 = 3;

    let scanner = login::QrCodeScanner::new(drive_config).await?;
    // 返回二维码内容结果集
    let sid = scanner.scan().await?.sid;
    // 需要生成二维码的内容
    let qrcode_content = format!("https://www.aliyundrive.com/o/oauth/authorize?sid={sid}");
    // 打印二维码
    qr2term::print_qr(&qrcode_content)?;
    info!("Please scan the qrcode to login in {} seconds", timeout);
    let loop_count = timeout / SLEEP;
    for _i in 0..loop_count {
        tokio::time::sleep(tokio::time::Duration::from_secs(SLEEP)).await;
        // 模拟轮训查询二维码状态
        let query_result = scanner.query(&sid).await?;
        if !query_result.is_success() {
            continue;
        }
        let code = query_result.auth_code.unwrap();
        let refresh_token = scanner.fetch_refresh_token(&code).await?;
        return Ok(refresh_token);
    }
    bail!("Login failed")
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
            bail!(err);
        }

        #[cfg(windows)]
        {
            let status = command.spawn().and_then(|mut c| c.wait())?;
            bail!("aliyundrive-webdav upgraded");
        }
    }
    Ok(())
}
