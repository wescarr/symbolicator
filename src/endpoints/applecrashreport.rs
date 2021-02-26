use futures::TryStreamExt;
use warp::{multipart::FormData, Filter, Rejection, Reply};

use crate::endpoints::symbolicate::SymbolicateParams;
use crate::services::Service;
use crate::types::RequestOptions;
use crate::utils::http::{self, BadRequest};
use crate::utils::multipart::{read_multipart_json, read_multipart_part};
use crate::utils::sentry::ConfigureScope;

async fn handle_apple_crash_report_request(
    params: SymbolicateParams,
    mut form_data: FormData,
    service: Service,
) -> Result<warp::reply::Json, Rejection> {
    sentry::start_session();
    params.configure_scope();

    let mut report = None;
    let mut sources = service.config().default_sources();
    let mut options = RequestOptions::default();

    while let Some(part) = form_data.try_next().await.map_err(BadRequest::from)? {
        match part.name() {
            "apple_crash_report" => report = Some(read_multipart_part(part).await?),
            "sources" => sources = read_multipart_json(part).await?,
            "options" => options = read_multipart_json(part).await?,
            _ => (), // Always ignore unknown fields.
        }
    }

    let report = report.ok_or_else(|| BadRequest::msg("missing apple crash report"))?;

    let symbolication = service.symbolication();
    let request_id =
        symbolication.process_apple_crash_report(params.scope, report, sources, options);

    match symbolication.get_response(request_id, params.timeout).await {
        Some(response) => Ok(warp::reply::json(&response)),
        None => Err(http::ServiceUnavailable.into()),
    }
}

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("applecrashreport")
        .and(warp::post())
        .and(warp::query())
        .and(warp::multipart::form().max_length(200 * 1024 * 1024))
        .and(http::with(service))
        .and_then(handle_apple_crash_report_request)
}

// #[cfg(test)]
// mod tests {
//     use std::fs;

//     use actix_web::test::TestServer;
//     use reqwest::{multipart, Client, StatusCode};

//     use crate::config::Config;
//     use crate::services::Service;
//     use crate::test;
//     use crate::types::SymbolicationResponse;

//     #[tokio::test]
//     async fn test_basic() {
//         test::setup();

//         let service = Service::create(Config::default()).unwrap();
//         let server = TestServer::with_factory(move || crate::server::create_app(service.clone()));

//         let file_contents = fs::read("tests/fixtures/apple_crash_report.txt").unwrap();
//         let file_part = multipart::Part::bytes(file_contents).file_name("apple_crash_report.txt");

//         let form = multipart::Form::new()
//             .part("apple_crash_report", file_part)
//             .text("sources", "[]");

//         let response = Client::new()
//             .post(&server.url("/applecrashreport"))
//             .multipart(form)
//             .send()
//             .await
//             .unwrap();

//         assert_eq!(response.status(), StatusCode::OK);

//         let body = response.text().await.unwrap();
//         let response = serde_json::from_str::<SymbolicationResponse>(&body).unwrap();
//         insta::assert_yaml_snapshot!(response);
//     }

//     #[tokio::test]
//     async fn test_unknown_field() {
//         test::setup();

//         let service = Service::create(Config::default()).unwrap();
//         let server = TestServer::with_factory(move || crate::server::create_app(service.clone()));

//         let file_contents = fs::read("tests/fixtures/apple_crash_report.txt").unwrap();
//         let file_part = multipart::Part::bytes(file_contents).file_name("apple_crash_report.txt");

//         let form = multipart::Form::new()
//             .part("apple_crash_report", file_part)
//             .text("sources", "[]")
//             .text("unknown", "value");

//         let response = Client::new()
//             .post(&server.url("/applecrashreport"))
//             .multipart(form)
//             .send()
//             .await
//             .unwrap();

//         assert_eq!(response.status(), StatusCode::OK);
//     }
// }
