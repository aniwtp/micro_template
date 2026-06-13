use flatbuffers::FlatBufferBuilder;
use ntex::{
    util::Bytes,
    web::{self, types::State},
};
use db_wrapper::DBWrapper;

use crate::{
    errors::AuthError,
    generated::{
        dto::login_generated::dto::login::{LoginRequest, TokenResponse, TokenResponseArgs},
        types::tokens_generated::types::{Bytes11, Bytes21, RSTokens, RSTokensArgs},
    },
};

fn serialize_token_response(refresh: [u8; 21], session: [u8; 11]) -> Vec<u8> {
    log::trace!(
        "serializing token response (refresh={}B, session={}B)",
        refresh.len(),
        session.len()
    );

    let mut builder = FlatBufferBuilder::new();

    let refresh_bytes = Bytes21::new(&refresh);
    let session_bytes = Bytes11::new(&session);

    let rs_tokens = RSTokens::create(
        &mut builder,
        &RSTokensArgs { refresh: Some(&refresh_bytes), session: Some(&session_bytes) },
    );

    let response =
        TokenResponse::create(&mut builder, &TokenResponseArgs { token: Some(rs_tokens) });

    builder.finish(response, None);
    let data = builder.finished_data().to_vec();
    log::debug!("token response serialized: {} bytes", data.len());
    data
}

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

    // Generate placeholder tokens
    let refresh_token = [0u8; 21];
    let session_token = [0u8; 11];

    let response_body = serialize_token_response(refresh_token, session_token);

    web::HttpResponse::Ok().content_type("application/octet-stream").body(response_body)
}

#[cfg(test)]
mod tests {
    use flatbuffers::FlatBufferBuilder;

    use crate::generated::dto::login_generated::dto::login::{
        LoginRequest, LoginRequestArgs, TokenResponse,
    };

    #[test]
    fn test_login_manual() {
        let addr = "http://localhost:8080/v1/auth/login";
        println!("→ POST {addr}");

        // --- Build FlatBuffer LoginRequest ---
        let mut builder = FlatBufferBuilder::new();
        let username = builder.create_string("alice");
        let password = builder.create_string("secret123");

        let req = LoginRequest::create(
            &mut builder,
            &LoginRequestArgs { username: Some(username), password: Some(password) },
        );
        builder.finish(req, None);
        let body = builder.finished_data().to_vec();
        println!("  request body: {body_len} bytes", body_len = body.len());

        // --- Send via reqwest (blocking) ---
        let client = reqwest::blocking::Client::new();
        let started = std::time::Instant::now();
        let resp =
            client.post(addr).body(body).send().expect("request failed — is the server running?");
        let elapsed = started.elapsed();

        let status = resp.status();
        println!(
            "← {} {}  ({elapsed:.2?})",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        );

        let resp_bytes = resp.bytes().expect("failed to read response body");
        println!("  response body: {} bytes", resp_bytes.len());

        // --- Try to parse FlatBuffer response (if 200 OK) ---
        if status.is_success() {
            match flatbuffers::root::<TokenResponse>(&resp_bytes) {
                Ok(token_resp) => {
                    println!("  ✓ FlatBuffer parsed: {token_resp:?}");
                },
                Err(e) => {
                    println!("  ✗ FlatBuffer parse error: {e}");
                },
            }
        } else {
            let text = String::from_utf8_lossy(&resp_bytes);
            println!("  body (text): {text}");
        }
    }
}
