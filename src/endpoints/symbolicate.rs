use std::convert::Infallible;

use serde::Deserialize;
use warp::reply::Response;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::services::symbolication::SymbolicateStacktraces;
use crate::services::Service;
use crate::sources::SourceConfig;
use crate::types::{RawObjectInfo, RawStacktrace, RequestOptions, Scope, Signal};
use crate::utils::{http, sentry::ConfigureScope};

/// Query parameters of the symbolication request.
#[derive(Deserialize)]
pub struct SymbolicateParams {
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub scope: Scope,
}

impl ConfigureScope for SymbolicateParams {
    fn to_scope(&self, scope: &mut sentry::Scope) {
        scope.set_tag("request.scope", &self.scope);
        if let Some(timeout) = self.timeout {
            scope.set_tag("request.timeout", timeout);
        } else {
            scope.set_tag("request.timeout", "none");
        }
    }
}

/// JSON body of the symbolication request.
#[derive(Deserialize)]
struct SymbolicateBody {
    #[serde(default)]
    pub signal: Option<Signal>,
    #[serde(default)]
    pub sources: Option<Vec<SourceConfig>>,
    #[serde(default)]
    pub stacktraces: Vec<RawStacktrace>,
    #[serde(default)]
    pub modules: Vec<RawObjectInfo>,
    #[serde(default)]
    pub options: RequestOptions,
}

async fn symbolicate(
    params: SymbolicateParams,
    body: SymbolicateBody,
    service: Service,
) -> Result<Response, Infallible> {
    sentry::start_session();
    params.configure_scope();

    let response_opt = service.compat().spawn_handle(async move {
        let sources = match body.sources {
            Some(sources) => sources.into(),
            None => service.config().default_sources(),
        };

        let symbolication = service.symbolication();
        let request_id = symbolication.symbolicate_stacktraces(SymbolicateStacktraces {
            scope: params.scope,
            signal: body.signal,
            sources,
            stacktraces: body.stacktraces,
            modules: body.modules.into_iter().map(From::from).collect(),
            options: body.options,
        });

        symbolication.get_response(request_id, params.timeout).await
    });

    Ok(match response_opt.await {
        Ok(Some(response)) => warp::reply::json(&response).into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    })
}

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("symbolicate")
        .and(warp::post())
        .and(warp::query())
        .and(warp::body::content_length_limit(5_000_000))
        .and(warp::body::json())
        .and(http::with(service))
        .and_then(symbolicate)
}
