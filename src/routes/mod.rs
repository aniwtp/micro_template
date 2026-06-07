mod auth;
use ntex::web;

pub fn routes(cfg: &mut web::ServiceConfig<web::DefaultError>) {
    log::trace!("configuring /v1 routes");
    cfg.service(
        web::scope("/v1").service(auth::scope()), // .service(users::scope())
    );
}
