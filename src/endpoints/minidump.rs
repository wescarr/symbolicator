use std::convert::Infallible;

use anyhow::{Context, Error};
use futures::TryStreamExt;
use warp::{http::StatusCode, multipart::FormData, reply::Response, Filter, Rejection, Reply};

use crate::endpoints::symbolicate::SymbolicateParams;
use crate::services::Service;
use crate::types::RequestOptions;
use crate::utils::http;
use crate::utils::multipart::{read_multipart_json, read_multipart_part};
use crate::utils::sentry::ConfigureScope;

async fn symbolicate_minidump(
    params: SymbolicateParams,
    mut form_data: FormData,
    state: Service,
) -> Result<Response, Error> {
    sentry::start_session();
    params.configure_scope();

    let mut minidump = None;
    let mut sources = state.config().default_sources();
    let mut options = RequestOptions::default();

    while let Some(part) = form_data.try_next().await? {
        match part.name() {
            "upload_file_minidump" => minidump = Some(read_multipart_part(part).await?),
            "sources" => sources = read_multipart_json(part).await?,
            "options" => options = read_multipart_json(part).await?,
            _ => (), // Always ignore unknown fields.
        }
    }

    let minidump = minidump.context("missing minidump")?;

    let symbolication = state.symbolication();
    let request_id = symbolication.process_minidump(params.scope, minidump, sources, options);

    let response = match symbolication.get_response(request_id, params.timeout).await {
        Some(response) => warp::reply::json(&response).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    Ok(response)
}

trait IntoHandler<Output, Args> {
    type Fn;
    fn into_handler(self) -> Self::Fn;
}

trait IntoTryHandler<Output, Args> {
    type Fn;
    fn into_try_handler(self) -> Self::Fn;
}

// impl<F, R> IntoHandler for F
// where
//     F: FnOnce() -> R + 'static,
// {
//     type Output = R;
//     type Fn = Box<dyn FnOnce() -> Result<Self::Output, Infallible>>;

//     fn into_handler(self) -> Self::Fn {
//         Box::new(move || Ok(self()))
//     }
// }

macro_rules! impl_into_handler {
    ($( $p:ident ),*) => {
        impl<Func, Output, $($p),* > IntoHandler<Output, ($($p,)*) > for Func
        where
            Func: FnOnce($($p),*) -> Output + 'static,
        {
            type Fn = Box<dyn FnOnce($($p),*) -> Result<Output, Infallible>>;

            fn into_handler(self) -> Self::Fn {
                Box::new(move |$($p),*| Ok(self($($p),*)))
            }
        }

        impl<Func, Output, $($p),* > IntoTryHandler<Output, ($($p,)*) > for Func
        where
            Func: FnOnce($($p),*) -> Result<Output, Error> + 'static,
        {
            type Fn = Box<dyn FnOnce($($p),*) -> Result<Output, Infallible>>;

            fn into_try_handler(self) -> Self::Fn {
                Box::new(move |$($p),*| Ok(self($($p),*).expect("TODO: convert error")))
            }
        }
    };
}

impl_into_handler!();
impl_into_handler!(A);
impl_into_handler!(A, B);
impl_into_handler!(A, B, C);
impl_into_handler!(A, B, C, D);
impl_into_handler!(A, B, C, D, E);
impl_into_handler!(A, B, C, D, E, F);
impl_into_handler!(A, B, C, D, E, F, G);
impl_into_handler!(A, B, C, D, E, F, G, H);

fn wrap_infallible<T, F>(f: F) -> Box<dyn FnOnce() -> Result<T, Infallible>>
where
    F: FnOnce() -> T + 'static,
{
    Box::new(move || Ok(f()))
}

fn handle<F, Response, Args>(f: F) -> <F as IntoHandler<Response, Args>>::Fn
where
    F: IntoHandler<Response, Args>,
{
    f.into_handler()
}

fn try_handle<F, Response, Args>(f: F) -> <F as IntoTryHandler<Response, Args>>::Fn
where
    F: IntoTryHandler<Response, Args>,
{
    f.into_try_handler()
}

fn service_unavailable() -> Response {
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("minidump")
        .and(warp::post())
        .and(warp::query())
        .and(warp::multipart::form().max_length(200 * 1024 * 1024))
        .and(http::with(service))
        .and_then(try_handle(symbolicate_minidump))
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

//         let file_contents = fs::read("tests/fixtures/windows.dmp").unwrap();
//         let file_part = multipart::Part::bytes(file_contents).file_name("windows.dmp");

//         let form = multipart::Form::new()
//             .part("upload_file_minidump", file_part)
//             .text("sources", "[]");

//         let response = Client::new()
//             .post(&server.url("/minidump"))
//             .multipart(form)
//             .send()
//             .await
//             .unwrap();

//         assert_eq!(response.status(), StatusCode::OK);

//         let body = response.text().await.unwrap();
//         let response = serde_json::from_str::<SymbolicationResponse>(&body).unwrap();
//         insta::assert_yaml_snapshot!(response);
//     }

//     // This test is disabled because it locks up on CI. We have not found a way to reproduce this.
//     #[allow(dead_code)]
//     // #[tokio::test]
//     async fn test_integration_microsoft() {
//         // TODO: Move this test to E2E tests
//         test::setup();

//         let service = Service::create(Config::default()).unwrap();
//         let server = TestServer::with_factory(move || crate::server::create_app(service.clone()));
//         let source = test::microsoft_symsrv();

//         let file_contents = fs::read("tests/fixtures/windows.dmp").unwrap();
//         let file_part = multipart::Part::bytes(file_contents).file_name("windows.dmp");

//         let form = multipart::Form::new()
//             .part("upload_file_minidump", file_part)
//             .text("sources", serde_json::to_string(&vec![source]).unwrap())
//             .text("options", r#"{"dif_candidates":true}"#);

//         let response = Client::new()
//             .post(&server.url("/minidump"))
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

//         let file_contents = fs::read("tests/fixtures/windows.dmp").unwrap();
//         let file_part = multipart::Part::bytes(file_contents).file_name("windows.dmp");

//         let form = multipart::Form::new()
//             .part("upload_file_minidump", file_part)
//             .text("sources", "[]")
//             .text("unknown", "value");

//         let response = Client::new()
//             .post(&server.url("/minidump"))
//             .multipart(form)
//             .send()
//             .await
//             .unwrap();

//         assert_eq!(response.status(), StatusCode::OK);
//     }
// }
