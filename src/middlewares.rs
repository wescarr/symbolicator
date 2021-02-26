use std::time::Instant;

use warp::path::FullPath;
use warp::{reply::Response, Filter, Rejection, Reply};

fn log_metrics(start_time: Instant, path: FullPath, reply: impl Reply) -> Response {
    let response = reply.into_response();

    if !path.as_str().starts_with("healthcheck") {
        metric!(timer("requests.duration") = start_time.elapsed());
        metric!(counter(&format!("responses.status_code.{}", response.status())) += 1);
    }

    response
}

pub fn metrics(
    f: impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Clone,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Clone {
    warp::any()
        .map(Instant::now)
        .and(warp::path::full())
        .and(f)
        .map(log_metrics)
}
