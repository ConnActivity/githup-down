use lambda_http::http::Method;
use lambda_http::{Body, Request, Response};
use std::error::Error;
use std::future::Future;
use tracing::info;

pub type BoxFut =
    std::pin::Pin<Box<dyn Future<Output = Result<Response<Body>, Box<dyn Error>>> + Send>>;
pub struct Route {
    pub method: Method,
    pub path: String,
    pub handler: fn(Request) -> BoxFut,
}
impl Route {
    pub fn new(method: Method, path: String, handler: fn(Request) -> BoxFut) -> Self {
        Self {
            method,
            path,
            handler,
        }
    }
    fn matches(&self, req: &Request) -> bool {
        self.method == req.method() && self.path == req.uri().path()
    }

    fn handle(&self, req: Request) -> BoxFut {
        (self.handler)(req)
    }
}

#[derive(Default)]
pub struct SmallRouter {
    pub(crate) routes: Vec<Route>,
}
impl SmallRouter {
    pub fn insert(&mut self, route: Route) {
        self.routes.push(route);
    }
    pub async fn route(&self, req: Request) -> Result<Response<Body>, Box<dyn Error>> {
        for route in &self.routes {
            if route.matches(&req) {
                info!("Matched route: {:?}", route.path);
                return route.handle(req).await;
            }
        }
        Ok(Response::builder()
            .status(404)
            .header("Content-Type", "text/plain")
            .body(Body::from("Not found"))
            .unwrap())
    }
}
