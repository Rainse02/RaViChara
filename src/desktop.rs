use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Icon, Window, WindowId};
use wry::{PageLoadEvent, WebContext, WebView, WebViewBuilder};

#[derive(Debug, Clone)]
enum DesktopEvent {
    Minimize,
    ToggleMaximize,
    Close,
    Drag,
    PageLoaded,
    BackendFailed(String),
}

fn application_icon() -> Option<Icon> {
    Icon::from_rgba(
        include_bytes!("../assets/ravichara-icon-64.rgba").to_vec(),
        64,
        64,
    )
    .ok()
}

struct DesktopApplication {
    webview: Option<WebView>,
    web_context: WebContext,
    window: Option<Window>,
    url: String,
    ipc_proxy: EventLoopProxy<DesktopEvent>,
    shutdown: Option<oneshot::Sender<()>>,
    smoke_test: bool,
    fatal_error: Option<String>,
    smoke_log: Option<PathBuf>,
}

impl DesktopApplication {
    fn record_smoke_state(&self, state: &str) {
        let Some(path) = self.smoke_log.as_ref() else {
            return;
        };
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{state}");
        }
    }

    fn close(&mut self, event_loop: &ActiveEventLoop) {
        self.record_smoke_state("closing");
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        // On Windows, WebView2 owns child HWNDs tied to the Winit event loop.
        // Release them before requesting loop termination; otherwise the
        // process can remain alive after the backend has already shut down.
        self.webview.take();
        self.window.take();
        self.record_smoke_state("window-and-webview-dropped");
        event_loop.exit();
        self.record_smoke_state("event-loop-exit-requested");
    }
}

impl ApplicationHandler<DesktopEvent> for DesktopApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.record_smoke_state("event-loop-resumed");
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("RaViChara — 虚拟角色伴侣")
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(LogicalSize::new(960.0, 640.0))
            .with_resizable(true)
            .with_decorations(false)
            .with_transparent(false)
            .with_window_icon(application_icon())
            // WebView2 initialization can block when its parent HWND has never
            // been made visible. Smoke-test mode hides it immediately after the
            // WebView has been created instead.
            .with_visible(true);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => window,
            Err(error) => {
                tracing::error!("failed to create desktop window: {error}");
                self.fatal_error = Some(format!("failed to create desktop window: {error}"));
                self.close(event_loop);
                return;
            }
        };
        self.record_smoke_state("window-created");

        let ipc_proxy = self.ipc_proxy.clone();
        let page_load_proxy = self.ipc_proxy.clone();
        let origin = self.url.clone();
        let builder = WebViewBuilder::new_with_web_context(&mut self.web_context)
            .with_url(&self.url)
            .with_autoplay(true)
            .with_hotkeys_zoom(false)
            .with_general_autofill_enabled(false)
            .with_devtools(cfg!(debug_assertions))
            .with_initialization_script(
                "Object.defineProperty(window, '__RAVICHARA_DESKTOP__', { value: true });",
            )
            .with_navigation_handler(move |url| {
                url.starts_with(&origin) || url == "about:blank"
            })
            .with_on_page_load_handler(move |event, _url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = page_load_proxy.send_event(DesktopEvent::PageLoaded);
                }
            })
            .with_ipc_handler(move |request| {
                if let Some(event) = parse_desktop_event(request.body()) {
                    let _ = ipc_proxy.send_event(event);
                }
            });
        let webview = match builder.build(&window) {
            Ok(webview) => webview,
            Err(error) => {
                tracing::error!("failed to create WebView2: {error}");
                self.fatal_error = Some(format!("failed to create WebView2: {error}"));
                self.close(event_loop);
                return;
            }
        };
        self.record_smoke_state("webview-created");
        if self.smoke_test {
            window.set_visible(false);
        }

        self.webview = Some(webview);
        self.window = Some(window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: DesktopEvent) {
        match event {
            DesktopEvent::PageLoaded => {
                self.record_smoke_state("page-load-finished");
                tracing::info!("desktop WebView finished loading {}", self.url);
                if self.smoke_test {
                    self.close(event_loop);
                }
                return;
            }
            DesktopEvent::BackendFailed(error) => {
                self.record_smoke_state("backend-failed");
                tracing::error!("desktop backend stopped: {error}");
                self.fatal_error = Some(format!("desktop backend stopped: {error}"));
                self.close(event_loop);
                return;
            }
            _ => {}
        }

        let Some(window) = self.window.as_ref() else {
            return;
        };
        match event {
            DesktopEvent::Minimize => window.set_minimized(true),
            DesktopEvent::ToggleMaximize => {
                window.set_maximized(!window.is_maximized());
            }
            DesktopEvent::Drag => {
                if let Err(error) = window.drag_window() {
                    tracing::debug!("window drag was not started: {error}");
                }
            }
            DesktopEvent::Close => self.close(event_loop),
            DesktopEvent::PageLoaded | DesktopEvent::BackendFailed(_) => unreachable!(),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(Window::id) != Some(window_id) {
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            self.close(event_loop);
        }
    }
}

fn parse_desktop_event(command: &str) -> Option<DesktopEvent> {
    match command.trim() {
        "minimize" => Some(DesktopEvent::Minimize),
        "maximize" => Some(DesktopEvent::ToggleMaximize),
        "close" => Some(DesktopEvent::Close),
        "drag" => Some(DesktopEvent::Drag),
        _ => None,
    }
}

pub fn run(runtime_root: PathBuf, smoke_test: bool) -> Result<(), Box<dyn Error>> {
    let (config, router) = crate::prepare_application()?;
    if config.server.host != "127.0.0.1" && config.server.host != "localhost" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "desktop mode requires server.host to be 127.0.0.1 or localhost",
        )
        .into());
    }
    let address = format!("{}:{}", config.server.host, config.server.port);
    let std_listener = std::net::TcpListener::bind(&address).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "无法监听 {address}。可能已有 RaViChara 实例正在运行：{error}"
            ),
        )
    })?;
    std_listener.set_nonblocking(true)?;

    let event_loop = EventLoop::<DesktopEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    if smoke_test {
        let timeout_proxy = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(15));
            let _ = timeout_proxy.send_event(DesktopEvent::BackendFailed(
                "desktop smoke test timed out while loading the WebView".into(),
            ));
        });
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let listener = {
        let _guard = runtime.enter();
        tokio::net::TcpListener::from_std(std_listener)?
    };
    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let backend = runtime.spawn(async move {
        tracing::info!(
            "RaViChara desktop v{} listening on http://{}",
            env!("CARGO_PKG_VERSION"),
            address
        );
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_receiver.await;
            })
            .await;
        if let Err(error) = result {
            let _ = proxy.send_event(DesktopEvent::BackendFailed(error.to_string()));
        }
    });

    let webview_directory = runtime_root.join("webview");
    let smoke_log = if smoke_test {
        let path = runtime_root.join("logs").join("desktop-smoke.log");
        let _ = std::fs::remove_file(&path);
        Some(path)
    } else {
        None
    };
    let url = format!("http://127.0.0.1:{}/", config.server.port);
    let mut application = DesktopApplication {
        webview: None,
        web_context: WebContext::new(Some(webview_directory)),
        window: None,
        url,
        ipc_proxy: event_loop.create_proxy(),
        shutdown: Some(shutdown_sender),
        smoke_test,
        fatal_error: None,
        smoke_log,
    };
    application.record_smoke_state("application-created");
    event_loop.run_app(&mut application)?;
    application.record_smoke_state("event-loop-returned");
    let fatal_error = application.fatal_error.take();
    drop(application);

    runtime.block_on(async {
        match tokio::time::timeout(Duration::from_secs(3), backend).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!("backend task join failed: {error}"),
            Err(_) => tracing::warn!("backend shutdown exceeded three seconds"),
        }
    });
    if let Some(error) = fatal_error {
        return Err(io::Error::other(error).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{application_icon, parse_desktop_event, DesktopEvent};

    #[test]
    fn embedded_window_icon_has_valid_rgba_dimensions() {
        assert!(application_icon().is_some());
    }

    #[test]
    fn parses_only_supported_window_commands() {
        assert!(matches!(
            parse_desktop_event(" minimize "),
            Some(DesktopEvent::Minimize)
        ));
        assert!(matches!(
            parse_desktop_event("maximize"),
            Some(DesktopEvent::ToggleMaximize)
        ));
        assert!(matches!(
            parse_desktop_event("close"),
            Some(DesktopEvent::Close)
        ));
        assert!(matches!(
            parse_desktop_event("drag"),
            Some(DesktopEvent::Drag)
        ));
        assert!(parse_desktop_event("open-external").is_none());
    }
}
