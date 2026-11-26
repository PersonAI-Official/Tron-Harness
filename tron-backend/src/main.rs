use std::{error::Error, future::IntoFuture, time::Duration};

use tokio::net::TcpListener;
use tron_backend::{
    app::{AppState, build_remote_router_with_state, build_router_with_state},
    config::AppConfig,
};

fn main() -> Result<(), Box<dyn Error>> {
    // Electron reads the SDK's platform codes before any backend is running.
    // This command must not load configuration or initialize storage.
    if std::env::args().nth(1).as_deref() == Some("--wsl-platforms") {
        use crab_serverapi::version::os_type;

        println!(
            "{}",
            serde_json::json!({
                "ubuntu": {
                    "x86_64": os_type::WSL_AMD64,
                    "aarch64": os_type::WSL_ARM64,
                },
                "debian": {
                    "x86_64": os_type::WSL_AMD64,
                    "aarch64": os_type::WSL_ARM64,
                },
            })
        );
        return Ok(());
    }
    // Load the compiled-in translation table once before any localized text is
    // produced. `rust-i18n` merges every `locales/*.yml` file at build time.
    tron_i18n::init();
    // Native dialogs must run on the process main thread on macOS. A short-lived
    // helper process keeps the HTTP server responsive without a GUI event loop.
    if std::env::args().nth(1).as_deref() == Some("--pick-directory") {
        let locale = tron_i18n::Locale::from_raw(std::env::args().nth(2).as_deref().unwrap_or(""));
        let title =
            tron_i18n::t!("common.native.pick_directory_title", locale = locale).into_owned();
        let selected = rfd::FileDialog::new().set_title(&title).pick_folder();
        let path = selected
            .as_ref()
            .map(|path| {
                path.to_str()
                    .ok_or_else(|| std::io::Error::other("directory path is not valid UTF-8"))
            })
            .transpose()?;
        println!("{}", serde_json::to_string(&path)?);
        return Ok(());
    }
    run_server()
}

#[tokio::main]
async fn run_server() -> Result<(), Box<dyn Error>> {
    let config = AppConfig::from_application_yaml()?;
    tron_backend::adapters::outbound::observability::logs::init_application_logging(&config)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let socket_addr = config.socket_addr()?;
    let listener = TcpListener::bind(socket_addr).await?;
    let state = AppState::new(config)?;
    state.init_remote_access().await?;
    let remote_access_addr = state.config.remote_access_listen_addr()?;
    let framework = state.agent_framework.clone();
    framework
        .observability()
        .await
        .map_err(|_| std::io::Error::other("initialize OTel collector"))?;
    let ingestion = state.knowledge_bases.clone();
    let router = build_router_with_state(state.clone())?;

    if state.remote_service().is_some() {
        // The supervisor starts, stops, and repairs the listener as remote
        // access settings change, so enabling it in the UI needs no restart.
        tron_backend::application::remote_service::ensure_remote_listener(state.clone()).await;
    } else if let Some(remote_addr) = remote_access_addr {
        let remote_router = build_remote_router_with_state(state.clone())?;
        match TcpListener::bind(remote_addr).await {
            Ok(remote_listener) => {
                state.set_remote_access_active(true);
                tracing::info!("tron-backend remote access listening on http://{remote_addr}");
                tokio::spawn(async move {
                    if let Err(error) = axum::serve(
                        remote_listener,
                        remote_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                    )
                    .await
                    {
                        tracing::error!("tron-backend remote access listener stopped: {error}");
                    }
                });
            }
            Err(error) => {
                tracing::error!(
                    "tron-backend remote access listener could not bind {remote_addr}: {error}"
                );
            }
        }
    }

    tracing::info!("tron-backend listening on http://{socket_addr}");
    let sqlite_maintenance = tokio::spawn(tron_backend::app::maintain_sqlite_databases());
    tokio::select! {
        // Finalize routes once; serving Router directly runs with_state(())
        // and clones every route's middleware state for each new connection.
        result = axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).into_future() => {
            result?;
            Ok(())
        }
        _ = shutdown_signal() => {
            sqlite_maintenance.abort();
            // A process exit must not depend on HTTP draining, a stuck callback,
            // or Tokio's blocking-pool teardown. The OS releases file locks on exit.
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(2));
                tracing::warn!("shutdown cleanup deadline reached; exiting process");
                std::process::exit(0);
            });
            tracing::warn!("shutdown requested: stopping ingestion, HTTP and Agent work");
            ingestion.stop_ingestion_workers();
            framework.cancel_all_runs();
            let pty = framework.pty();
            let (report, terminal) = tokio::join!(
                framework.close(),
                tokio::task::spawn_blocking(move || pty.close()),
            );
            tracing::warn!("shutdown cleanup finished: {report:?}; terminal={terminal:?}");
            std::process::exit(0);
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
