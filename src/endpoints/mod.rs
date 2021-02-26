use warp::{Filter, Rejection, Reply};

use crate::services::Service;

mod applecrashreport;
mod healthcheck;
mod minidump;
mod proxy;
mod requests;
mod symbolicate;

pub fn filter(service: Service) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    healthcheck::filter()
        .or(symbolicate::filter(service.clone()))
        .or(minidump::filter(service.clone()))
        .or(applecrashreport::filter(service.clone()))
        .or(requests::filter(service.clone()))
        .or(proxy::filter(service))
}
