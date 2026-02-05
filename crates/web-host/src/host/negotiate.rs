// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Content negotiation for dual-format (FlatBuffers + JSON) API responses.

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use moor_schema::rpc as moor_rpc;
use planus::ReadAsRoot;
use tracing::error;

pub const FLATBUFFERS_CONTENT_TYPE: &str = "application/x-flatbuffers";
pub const JSON_CONTENT_TYPE: &str = "application/json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    FlatBuffers,
    Json,
}

/// Parse an Accept header and determine the response format.
///
/// - `None` / `*/*` → return `default`
/// - `application/x-flatbuffers` → FlatBuffers
/// - `application/json` → Json
/// - Multiple types → first supported match
/// - No match → 406 Not Acceptable
pub fn negotiate_response_format(
    accept: Option<&HeaderValue>,
    supported: &[ResponseFormat],
    default: ResponseFormat,
) -> Result<ResponseFormat, StatusCode> {
    let accept_str = match accept {
        None => return Ok(default),
        Some(val) => match val.to_str() {
            Ok(s) => s,
            Err(_) => return Ok(default),
        },
    };

    // Parse comma-separated Accept values (ignoring q-factors for RC1)
    for media_type in accept_str.split(',') {
        let media_type = media_type.trim();
        // Strip any q-factor parameters (e.g., ";q=0.9")
        let media_type = media_type
            .split(';')
            .next()
            .unwrap_or(media_type)
            .trim();

        if media_type == "*/*" || media_type.eq_ignore_ascii_case("application/*") {
            return Ok(default);
        }

        if media_type.eq_ignore_ascii_case(FLATBUFFERS_CONTENT_TYPE)
            && supported.contains(&ResponseFormat::FlatBuffers)
        {
            return Ok(ResponseFormat::FlatBuffers);
        }

        if media_type.eq_ignore_ascii_case(JSON_CONTENT_TYPE)
            && supported.contains(&ResponseFormat::Json)
        {
            return Ok(ResponseFormat::Json);
        }
    }

    Err(StatusCode::NOT_ACCEPTABLE)
}

/// Build a FlatBuffers response with correct Content-Type.
pub fn flatbuffer_response(reply_bytes: Vec<u8>) -> Response {
    match Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", FLATBUFFERS_CONTENT_TYPE)
        .body(Body::from(reply_bytes))
    {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to build FlatBuffer response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Build a JSON response from a serializable value.
pub fn json_response<T: serde::Serialize>(value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(bytes) => match Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", JSON_CONTENT_TYPE)
            .body(Body::from(bytes))
        {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to build JSON response: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Err(e) => {
            error!("Failed to serialize JSON: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Convert raw FlatBuffer bytes containing a ReplyResult to a JSON response.
pub fn reply_result_to_json(fb_bytes: &[u8]) -> Result<Response, StatusCode> {
    let ref_val = moor_rpc::ReplyResultRef::read_as_root(fb_bytes).map_err(|e| {
        error!("Failed to read ReplyResult from FlatBuffer: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let owned = moor_rpc::ReplyResult::try_from(ref_val).map_err(|e| {
        error!("Failed to convert ReplyResult to owned: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(json_response(&owned))
}

/// Convert a VerbCallResponse (already built as owned type) to a JSON response.
pub fn verb_call_response_to_json(
    response: &moor_rpc::VerbCallResponse,
) -> Result<Response, StatusCode> {
    Ok(json_response(response))
}

/// Both formats supported, defaulting to FlatBuffers.
pub const BOTH_FORMATS: &[ResponseFormat] =
    &[ResponseFormat::FlatBuffers, ResponseFormat::Json];

pub const TEXT_PLAIN_CONTENT_TYPE: &str = "text/plain";

/// Validate that the request Content-Type matches one of the expected types.
/// Returns `Ok(())` if the header is present and matches, or `Err(415)` otherwise.
/// An absent Content-Type is accepted when `allow_missing` is true (for
/// backwards compatibility with clients that omit it on plain-text bodies).
pub fn require_content_type(
    content_type: Option<&HeaderValue>,
    expected: &[&str],
    allow_missing: bool,
) -> Result<(), StatusCode> {
    let ct = match content_type {
        None if allow_missing => return Ok(()),
        None => return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
        Some(val) => match val.to_str() {
            Ok(s) => s,
            Err(_) => return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
        },
    };
    // Strip parameters (e.g. "; charset=utf-8")
    let media_type = ct.split(';').next().unwrap_or(ct).trim();
    if expected
        .iter()
        .any(|e| media_type.eq_ignore_ascii_case(e))
    {
        Ok(())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hv(s: &str) -> HeaderValue {
        HeaderValue::from_str(s).unwrap()
    }

    #[test]
    fn no_accept_returns_default() {
        let result = negotiate_response_format(None, BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::FlatBuffers));

        let result = negotiate_response_format(None, BOTH_FORMATS, ResponseFormat::Json);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }

    #[test]
    fn wildcard_returns_default() {
        let accept = hv("*/*");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::FlatBuffers));
    }

    #[test]
    fn explicit_flatbuffers() {
        let accept = hv("application/x-flatbuffers");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::Json);
        assert_eq!(result, Ok(ResponseFormat::FlatBuffers));
    }

    #[test]
    fn explicit_json() {
        let accept = hv("application/json");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }

    #[test]
    fn unsupported_type_returns_406() {
        let accept = hv("text/html");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Err(StatusCode::NOT_ACCEPTABLE));
    }

    #[test]
    fn json_not_supported_returns_406() {
        let accept = hv("application/json");
        let fb_only = &[ResponseFormat::FlatBuffers];
        let result =
            negotiate_response_format(Some(&accept), fb_only, ResponseFormat::FlatBuffers);
        assert_eq!(result, Err(StatusCode::NOT_ACCEPTABLE));
    }

    #[test]
    fn multiple_types_first_match_wins() {
        let accept = hv("application/json, application/x-flatbuffers");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }

    #[test]
    fn multiple_types_with_wildcard() {
        let accept = hv("text/html, */*");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::FlatBuffers));
    }

    #[test]
    fn q_factor_stripped() {
        let accept = hv("application/json;q=0.9");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }

    #[test]
    fn case_insensitive() {
        let accept = hv("Application/JSON");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }

    #[test]
    fn require_ct_match() {
        let ct = hv("application/x-flatbuffers");
        assert!(require_content_type(Some(&ct), &[FLATBUFFERS_CONTENT_TYPE], false).is_ok());
    }

    #[test]
    fn require_ct_with_charset() {
        let ct = hv("text/plain; charset=utf-8");
        assert!(require_content_type(Some(&ct), &[TEXT_PLAIN_CONTENT_TYPE], false).is_ok());
    }

    #[test]
    fn require_ct_mismatch_returns_415() {
        let ct = hv("text/html");
        assert_eq!(
            require_content_type(Some(&ct), &[FLATBUFFERS_CONTENT_TYPE], false),
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        );
    }

    #[test]
    fn require_ct_missing_allowed() {
        assert!(require_content_type(None, &[FLATBUFFERS_CONTENT_TYPE], true).is_ok());
    }

    #[test]
    fn require_ct_missing_rejected() {
        assert_eq!(
            require_content_type(None, &[FLATBUFFERS_CONTENT_TYPE], false),
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        );
    }

    #[test]
    fn application_wildcard_returns_default() {
        let accept = hv("application/*");
        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::FlatBuffers);
        assert_eq!(result, Ok(ResponseFormat::FlatBuffers));

        let result =
            negotiate_response_format(Some(&accept), BOTH_FORMATS, ResponseFormat::Json);
        assert_eq!(result, Ok(ResponseFormat::Json));
    }
}
