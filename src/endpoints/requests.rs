use std::convert::Infallible;

use serde::Deserialize;
use warp::{http::StatusCode, reply::Response, Filter, Rejection, Reply};

use crate::services::Service;
use crate::types::RequestId;
use crate::utils::http;

/// Query parameters of the symbolication poll request.
#[derive(Deserialize)]
struct PollRequestParams {
    #[serde(default)]
    pub timeout: Option<u64>,
}

async fn poll_request(
    request_id: RequestId,
    params: PollRequestParams,
    service: Service,
) -> Result<Response, Infallible> {
    let response_future = service
        .symbolication()
        .get_response(request_id, params.timeout);

    Ok(match service.spawn_compat(response_future).await {
        Ok(Some(response)) => warp::reply::json(&response).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_canceled) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    })
}

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("requests" / RequestId)
        .and(warp::get())
        .and(warp::query())
        .and(http::with(service))
        .and_then(poll_request)
}

#[cfg(test)]
mod tests {
    use warp::http::StatusCode;
    use warp::test::request;

    use crate::config::Config;
    use crate::services::Service;
    use crate::types::RequestId;

    #[tokio::test]
    async fn test_not_found() {
        let service = Service::create(Config::default()).unwrap();
        let endpoint = super::filter(service);

        let request_id = RequestId::new(Default::default());
        let response = request()
            .method("GET")
            .path(&format!("/requests/{}", request_id))
            .reply(&endpoint)
            .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
