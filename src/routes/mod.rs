mod auth;
use ntex::web;

pub fn routes(cfg: &mut web::ServiceConfig<web::DefaultError>) {
    cfg.service(
        web::scope("/v1").service(auth::scope()), // .service(users::scope())
    );
}
