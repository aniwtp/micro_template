//! Enum-based converters from domain types to FlatBuffer bytes.
//!
//! Each response variant encapsulates the serialisation logic — handlers just
//! construct the enum and call `.to_flatbuffer()`.

use flatbuffers::FlatBufferBuilder;
use rust_flatbuffer_macros::build_flatbuffer;

use crate::generated::login_generated::{
    dto::login::{TokenResponse, TokenResponseArgs},
    types::{Bytes11, Bytes21, RSTokens, RSTokensArgs},
};

/// Login response variants.
pub enum LoginReply {
    /// Authentication successful — returns refresh + session tokens.
    Success { refresh: [u8; 21], session: [u8; 11] },
}

impl LoginReply {
    /// Serialise this reply into a FlatBuffer byte vector ready for the wire.
    pub fn to_flatbuffer(&self) -> Vec<u8> {
        match self {
            Self::Success { refresh, session } => {
                log::trace!(
                    "serializing login reply (refresh={}B, session={}B)",
                    refresh.len(),
                    session.len()
                );

                #[allow(clippy::needless_update)]
                {
                    let mut builder = FlatBufferBuilder::new();
                    let refresh = Some(&Bytes21::new(refresh));
                    let session = Some(&Bytes11::new(session));
                    let rs_tokens =
                        build_flatbuffer!(&mut builder, RSTokens, refresh, session);
                    let token = Some(rs_tokens);
                    let response =
                        build_flatbuffer!(&mut builder, TokenResponse, token);

                    builder.finish(response, None);
                    let data = builder.finished_data().to_vec();
                    log::debug!("login reply serialized: {} bytes", data.len());
                    data
                }
            },
        }
    }
}
