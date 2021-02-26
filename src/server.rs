use std::net::SocketAddr;

use anyhow::{Context, Result};
use warp::{Filter, Rejection, Reply};

use crate::config::Config;
use crate::endpoints;
use crate::middlewares;
use crate::services::Service;

#[inline]
pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    endpoints::filter(service)
        .with(warp::log(module_path!()))
        .with(warp::wrap_fn(middlewares::metrics))
}

/// Starts all actors and HTTP server based on loaded config.
pub fn run(config: Config) -> Result<()> {
    // Log this metric before actually starting the server. This allows to see restarts even if
    // service creation fails. The HTTP server is bound before the actix system runs.
    metric!(counter("server.starting") += 1);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("symbolicator")
        .enable_all()
        .build()
        .unwrap();

    let bind = config.bind.clone();

    // Enter the tokio runtime before creating the services.
    let _guard = runtime.enter();
    let service = Service::create(config).context("failed to create service state")?;

    log::info!("Starting http server: {}", bind);
    let socket = bind.parse::<SocketAddr>()?;
    let server = warp::serve(filter(service)).bind(socket);
    runtime.block_on(server);
    log::info!("System shutdown complete");

    Ok(())
}
