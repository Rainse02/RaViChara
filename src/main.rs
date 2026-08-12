#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod blender;
mod config;
#[cfg(target_os = "windows")]
mod desktop;
mod http_client;
mod llm;
mod mcp;
mod memory;
mod persona;
mod realtime;
mod runtime;
mod server;
mod voice;

use config::AppConfig;
use persona::PersonaCard;
use server::AppState;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

fn main() {
    if let Err(error) = run() {
        let message = format!("RaViChara 启动失败：\n\n{error}");
        eprintln!("{message}");
        show_fatal_error(&message);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let server_only = std::env::args().any(|argument| argument == "--server");
    let desktop_smoke_test =
        std::env::args().any(|argument| argument == "--desktop-smoke-test");
    let explicit_home = parse_data_dir_argument()?;
    let runtime_root = runtime::prepare_runtime_root(explicit_home.as_deref())?;
    init_logging(&runtime_root)?;
    tracing::info!("runtime root: {}", runtime_root.display());

    #[cfg(target_os = "windows")]
    if !server_only {
        return desktop::run(runtime_root, desktop_smoke_test);
    }

    run_server_only()
}

fn parse_data_dir_argument() -> Result<Option<std::path::PathBuf>, Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--data-dir" {
            let value = arguments.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--data-dir requires a directory path",
                )
            })?;
            return Ok(Some(std::path::PathBuf::from(value)));
        }
    }
    Ok(None)
}

fn init_logging(runtime_root: &Path) -> Result<(), Box<dyn Error>> {
    let log_directory = runtime_root.join("logs");
    fs::create_dir_all(&log_directory)?;
    let log_path = log_directory.join("ravichara.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let shared = Arc::new(file);
    let writer = move || {
        shared.try_clone().unwrap_or_else(|_| {
            OpenOptions::new()
                .write(true)
                .open(null_device())
                .expect("the operating-system null device must be available")
        })
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,ravichara=debug".into()),
        )
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Ok(())
}

fn null_device() -> &'static str {
    if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn prepare_application() -> Result<(AppConfig, axum::Router), Box<dyn Error>> {
    let config = AppConfig::load().map_err(io::Error::other)?;
    let persona = PersonaCard::load_from_file(&config.character.card)
        .map_err(|error| io::Error::other(format!("加载角色卡失败：{error}")))?;
    let state = AppState::new(config.clone(), persona).map_err(io::Error::other)?;
    let router = server::create_router(state);
    Ok((config, router))
}

fn run_server_only() -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let (config, router) = prepare_application()?;
        let address = format!("{}:{}", config.server.host, config.server.port);
        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("无法监听 {address}：{error}"),
                )
            })?;

        tracing::info!(
            "RaViChara v{} listening on http://{}",
            env!("CARGO_PKG_VERSION"),
            address
        );
        axum::serve(listener, router)
            .await
            .map_err(|error| io::Error::other(format!("HTTP 服务异常退出：{error}")))?;
        Ok::<(), Box<dyn Error>>(())
    })
}

#[cfg(target_os = "windows")]
fn show_fatal_error(message: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK,
    };

    let title = std::ffi::OsStr::new("RaViChara")
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let body = std::ffi::OsStr::new(message)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_fatal_error(_message: &str) {}
