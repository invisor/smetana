//! Process entry point: read the environment, build the provider clients, serve.

use std::net::SocketAddr;
use std::sync::Arc;

use push_gateway::{app, config::Config, providers, RateLimiter, State};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "push_gateway=info".into()),
        )
        .with_target(false)
        .init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuration error: {e}");
            std::process::exit(2);
        }
    };

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("http client");

    let apns = config.apns.map(|c| {
        Arc::new(providers::apns::Apns::new(http.clone(), c)) as Arc<dyn providers::Provider>
    });
    let fcm = config.fcm.map(|c| {
        Arc::new(providers::fcm::Fcm::new(http.clone(), c)) as Arc<dyn providers::Provider>
    });
    tracing::info!(ios = apns.is_some(), android = fcm.is_some(), "providers configured");

    let state = State {
        apns,
        fcm,
        limiter: Arc::new(RateLimiter::new(
            config.rate_limit_per_minute,
            std::time::Duration::from_secs(60),
        )),
        trust_forwarded_for: config.trust_forwarded_for,
        body_timeout: std::time::Duration::from_secs(5),
    };

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .expect("bind");
    tracing::info!("listening");
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .expect("serve");
}
