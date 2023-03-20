use dav_server::{memls::MemLs, DavHandler};
use egui_notify::Toasts;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::drive::{AliyunDrive, DriveConfig};
use crate::vfs::AliyunDriveFileSystem;
use crate::webdav::WebDavServer;

#[derive(Deserialize, Serialize)]
struct App {
    #[serde(skip)]
    toasts: Toasts,
    #[serde(skip)]
    drive_config: DriveConfig,
    #[serde(skip)]
    join_handle: Option<JoinHandle<()>>,
    #[serde(skip)]
    refresh_token: String,
    root: String,
    host: String,
    port: String,
    redirect: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            toasts: Toasts::new(),
            drive_config: DriveConfig::default(),
            join_handle: None,
            refresh_token: String::new(),
            root: "/".to_string(),
            host: "0.0.0.0".to_string(),
            port: "8080".to_string(),
            redirect: false,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            App::default()
        };
        if let Some(workdir) = app.drive_config.workdir.as_ref() {
            let file = workdir.join("refresh_token");
            let refresh_token = std::fs::read_to_string(file).unwrap_or_default();
            app.refresh_token = refresh_token;
        }
        app
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("aliyundrive-webdav settings");
            ui.separator();
            egui::Grid::new("settings-grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.hyperlink_to("Refresh token", "https://messense-aliyundrive-webdav-backendrefresh-token-ucs0wn.streamlit.app/");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.refresh_token);
                    });
                    ui.end_row();
                    ui.label("Root directory");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.root);
                    });
                    ui.end_row();
                    ui.label("Listen host");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.host);
                    });
                    ui.end_row();
                    ui.label("Listen port");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.port);
                    });
                    ui.end_row();
                    ui.label("Enable 302 redirect");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.redirect, "");
                    });
                    ui.end_row();
                });
            ui.separator();
            if self.join_handle.is_some() {
                ui.horizontal(|ui| {
                    ui.label("Running at: ");
                    ui.hyperlink(format!("http://{}:{}", self.host, self.port));
                });
            } else if ui.button("Start!").clicked() {
                if self.refresh_token.is_empty() {
                    self.toasts.error("Refresh token is empty.");
                    return;
                } else if self.refresh_token.split('.').count() < 3 {
                    self.toasts.error("Refresh token is invalid.");
                    return;
                }
                let Ok(port) = self.port.parse::<u16>() else {
                        self.toasts.error("Listen port is invalid.");
                        return;
                    };
                let host = self.host.clone();
                let root = self.root.clone();
                let redirect = self.redirect;
                let drive_config = self.drive_config.clone();
                let refresh_token = self.refresh_token.clone();
                let join_handle = tokio::spawn(async move {
                    let drive = AliyunDrive::new(drive_config.clone(), refresh_token)
                        .await
                        .unwrap();
                    let fs = AliyunDriveFileSystem::new(drive, root, 1000, 600).unwrap();
                    let dav_server_builder = DavHandler::builder()
                        .filesystem(Box::new(fs))
                        .locksystem(MemLs::new())
                        // .read_buf_size(opt.read_buffer_size)
                        .autoindex(true)
                        .redirect(redirect);
                    let dav_server = dav_server_builder.build_handler();
                    let server = WebDavServer {
                        host,
                        port,
                        auth_user: None,
                        auth_password: None,
                        tls_config: None,
                        handler: dav_server,
                    };
                    server.serve().await.unwrap();
                });
                self.join_handle = Some(join_handle);
                self.toasts.success("Started aliyundrive-webdav.");
            }

            self.toasts.show(ctx);
        });
    }
}

pub fn run() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(400.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "aliyundrive-webdav",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
}
