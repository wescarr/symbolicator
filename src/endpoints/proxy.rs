use std::convert::Infallible;
use std::sync::Arc;

use http::InternalServerError;
use warp::{http::HeaderValue, hyper::Body, reply::Response, Filter, Rejection, Reply};

use crate::services::objects::{FindObject, ObjectHandle, ObjectPurpose};
use crate::services::Service;
use crate::types::Scope;
use crate::utils::{http, paths::parse_symstore_path};

async fn load_object(
    state: &Service,
    path: &str,
) -> Result<Option<Arc<ObjectHandle>>, InternalServerError> {
    let config = state.config();
    if !config.symstore_proxy {
        return Ok(None);
    }

    let (filetypes, object_id) = match parse_symstore_path(path) {
        Some(tuple) => tuple,
        None => return Ok(None),
    };

    log::debug!("Searching for {:?} ({:?})", object_id, filetypes);

    let found_object = state
        .objects()
        .find(FindObject {
            filetypes,
            identifier: object_id,
            sources: config.default_sources(),
            scope: Scope::Global,
            purpose: ObjectPurpose::Debug,
        })
        .await
        // TODO(ja): Bring back contexts, figure out error handling
        // .context("failed to download object")?;
        ?;

    let object_meta = match found_object.meta {
        Some(meta) => meta,
        None => return Ok(None),
    };

    let object_handle = state
        .objects()
        .fetch(object_meta)
        .await
        // .context("failed to download object")?;
        ?;

    if object_handle.has_object() {
        Ok(Some(object_handle))
    } else {
        Ok(None)
    }
}

async fn proxy_symstore_request(
    method: warp::http::Method,
    path: warp::path::Tail,
    service: Service,
) -> Result<Response, Rejection> {
    let is_head = match method {
        warp::http::Method::HEAD => true,
        warp::http::Method::GET => false,
        _ => return Ok(warp::http::StatusCode::METHOD_NOT_ALLOWED.into_response()),
    };

    let object_handle = match load_object(&service, path.as_str()).await? {
        Some(handle) => handle,
        // TODO: This shouldn't be a rejection
        // https://github.com/seanmonstar/warp/issues/712#issuecomment-697031645
        None => return Err(warp::reject::not_found()),
    };

    let len = object_handle.len();
    let mut response = if is_head {
        Response::new(Body::empty())
    } else {
        // TODO(ja): Framed read via tokio-util?
        // let bytes = Cursor::new(object_handle.data());
        // let async_bytes = FramedRead::new(bytes, BytesCodec::new()).map(|bytes| bytes.freeze());
        Response::new(Body::wrap_stream(futures::stream::once(async move {
            Ok::<_, Infallible>(object_handle.data().to_vec())
        })))
    };

    let headers = response.headers_mut();
    headers.insert("content-length", len.into());
    headers.insert(
        "content-type",
        HeaderValue::from_static("application/octet-stream"),
    );

    Ok(response)
}

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("requests")
        .and(warp::method())
        .and(warp::path::tail())
        .and(http::with(service))
        .and_then(proxy_symstore_request)
}
