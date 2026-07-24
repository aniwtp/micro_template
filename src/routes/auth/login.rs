use ntex::{
    util::Bytes,
    web::{self, types::State, HttpResponse},
};
use db_wrapper::DBWrapper;

use crate::{errors::AppError, LoginReply};

#[web::post("/login")]
async fn user_login(_db: State<DBWrapper>, body: Bytes) -> Result<HttpResponse, AppError> {
    use crate::utils::generated::login_generated::dto::login::LoginRequest;

    log::debug!("POST /login — request received ({} bytes)", body.len());

    let login = flatbuffers::root::<LoginRequest>(body.as_ref())?;

    let username = login.username().unwrap_or("<none>");
    let _password = login.password().unwrap_or("<none>");
    log::trace!("login attempt: username=`{username}`");

    if username == "<none>" || username.is_empty() {
        return Err(crate::errors::AuthError::MissingField("username".into()).into());
    }

    log::info!("login successful for `{username}`");

    let reply = LoginReply::Success { refresh: [0u8; 21], session: [0u8; 11] };
    Ok(HttpResponse::Ok().content_type("application/octet-stream").body(reply.to_flatbuffer()))
}
