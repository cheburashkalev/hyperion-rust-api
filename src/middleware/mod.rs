use actix_web::body::MessageBody;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform}, http::{header::ContentType, StatusCode}
    ,
    Error,
    HttpResponse
    ,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use serde::Serialize;
use std::rc::Rc;

#[derive(Serialize)]
struct NotFoundResponse {
    message: String,
    error: &'static str,
    #[serde(rename = "statusCode")]
    status_code: u16,
}

pub struct NotFoundMiddleware;

impl<S, B> Transform<S, ServiceRequest> for NotFoundMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = NotFoundMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(NotFoundMiddlewareService {
            service: Rc::new(service),
        })
    }
}

pub struct NotFoundMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for NotFoundMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<ServiceResponse, Error>>;

    fn poll_ready(&self, ctx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        let method = req.method().clone();
        let path = req.path().to_owned();
        Box::pin(async move {

            let res = service.call(req).await;
            match res {
                Ok(res) => {
                    if res.status() == StatusCode::NOT_FOUND {
                        let json = NotFoundResponse {
                            message: format!("Route {}:{} not found", method, path),
                            error: "Not Found",
                            status_code: 404,
                        };

                        let new_res = res.into_response(
                            HttpResponse::NotFound()
                                .insert_header(ContentType::json())
                                .body(serde_json::to_string(&json).unwrap())
                                .map_into_boxed_body(),
                        );

                        return Ok(new_res);
                    }
                    Ok(res.map_into_boxed_body())
                }
                Err(e) => {return Err(e);
                }
            }
        })
    }
}
