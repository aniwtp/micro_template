use flatbuffers::FlatBufferBuilder;
use ntex::{
    util::Bytes,
    web::{self, types::State},
};

use crate::{
    bd::DBWrapper,
    generated::{
        dto::login_generated::dto::login::{
            LoginRequest, TokenResponse, TokenResponseArgs,
        },
        types::tokens_generated::types::{Bytes11, Bytes21, RSTokens, RSTokensArgs},
    },
};

fn serialize_token_response(refresh: [u8; 21], session: [u8; 11]) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let refresh_bytes = Bytes21::new(&refresh);
    let session_bytes = Bytes11::new(&session);

    let rs_tokens = RSTokens::create(
        &mut builder,
        &RSTokensArgs {
            refresh: Some(&refresh_bytes),
            session: Some(&session_bytes),
        },
    );

    let response = TokenResponse::create(
        &mut builder,
        &TokenResponseArgs {
            token: Some(rs_tokens),
        },
    );

    builder.finish(response, None);
    builder.finished_data().to_vec()
}
#[web::post("/login")]
async fn user_login(_db: State<DBWrapper>, body: Bytes) -> impl web::Responder {
    let _login = flatbuffers::root::<LoginRequest>(body.as_ref()).unwrap();
    web::HttpResponse::Ok().body(serialize_token_response([0; 21], [0; 11]))
}
