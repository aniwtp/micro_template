use ntex::{
    util::Bytes,
    web::{self, types::State},
};
use db_wrapper::DBWrapper;

use crate::{
    errors::AuthError,
    utils::generated::login_generated::dto::login::LoginRequest,
    LoginReply,
};

#[web::post("/login")]
async fn user_login(_db: State<DBWrapper>, body: Bytes) -> impl web::Responder {
    log::debug!("POST /login — request received ({} bytes)", body.len());

    // Parse flatbuffer request
    let login = match flatbuffers::root::<LoginRequest>(body.as_ref()) {
        Ok(req) => {
            log::debug!("login request parsed successfully");
            req
        },
        Err(e) => {
            let err = AuthError::MalformedRequest(e.to_string());
            log::warn!("login: {err}");
            return web::HttpResponse::BadRequest().body(err.to_string());
        },
    };

    // Read fields from flatbuffer (placeholder — actual auth logic goes here)
    let username = login.username().unwrap_or("<none>");
    let _password = login.password().unwrap_or("<none>");
    log::trace!("login attempt: username=`{username}`");

    if username == "<none>" || username.is_empty() {
        let err = AuthError::MissingField("username".into());
        log::warn!("login: {err}");
        return web::HttpResponse::BadRequest().body(err.to_string());
    }

    log::info!("login successful for `{username}`");

    // Generate placeholder tokens and serialise via enum converter
    let reply = LoginReply::Success {
        refresh: [0u8; 21],
        session: [0u8; 11],
    };
    let response_body = reply.to_flatbuffer();

    web::HttpResponse::Ok().content_type("application/octet-stream").body(response_body)
}
