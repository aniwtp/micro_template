use ntex::web::{self, DefaultError, Scope};

mod login;

pub fn scope() -> Scope<DefaultError> {
    web::scope("/auth").service(login::user_login)
}
