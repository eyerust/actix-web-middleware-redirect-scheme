use actix_service::{forward_ready, Service};
use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    http::{
        self,
        header::{self, HeaderValue},
        StatusCode,
    },
    Error, HttpResponse,
};
use futures::future::{ok, Either, LocalBoxFuture, Ready};
use log::{debug, info};
use std::task::{Context, Poll};

pub struct RedirectSchemeService<S> {
    pub service: S,
    pub disable: bool,
    pub https_to_http: bool,
    pub temporary: bool,
    pub replacements: Vec<(String, String)>,
}

impl<S, B> Service<ServiceRequest> for RedirectSchemeService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if self.disable
            || (!self.https_to_http && req.connection_info().scheme() == "https")
            || (self.https_to_http && req.connection_info().scheme() == "http")
        {
            debug!("Not redirecting");
            Box::pin(self.service.call(req))
        } else {
            let host = req.connection_info().host().to_owned();
            let uri = req.uri().to_owned();
            let mut url = if self.https_to_http {
                format!("http://{}{}", host, uri)
            } else {
                format!("https://{}{}", host, uri)
            };
            for (s1, s2) in self.replacements.iter() {
                url = url.replace(s1, s2);
            }

            let status = if self.temporary {
                StatusCode::TEMPORARY_REDIRECT
            } else {
                StatusCode::MOVED_PERMANENTLY
            };

            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                let (request, response) = res.into_parts();

                let mut response = HttpResponse::with_body(status, response.into_body());
                response
                    .headers_mut()
                    .insert(header::LOCATION, HeaderValue::from_str(&url).unwrap());
                info!("Redirected to {}", url);

                Ok(ServiceResponse::new(request, response))
            })
        }
    }
}
