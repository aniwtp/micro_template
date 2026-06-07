use ntex::web::{self, DefaultError, Scope};

mod login;

pub fn scope() -> Scope<DefaultError> {
    log::trace!("configuring /auth routes");
    web::scope("/auth").service(login::user_login)
}
