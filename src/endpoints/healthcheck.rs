use warp::{Filter, Rejection, Reply};

fn healthcheck() -> &'static str {
    metric!(counter("healthcheck") += 1);
    "ok"
}

pub fn filter() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("healthcheck").and(warp::get()).map(healthcheck)
}

#[cfg(test)]
mod tests {
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_ok() {
        let endpoint = super::filter();

        let response = request()
            .method("GET")
            .path("/healthcheck")
            .reply(&endpoint)
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), "ok");
    }
}
