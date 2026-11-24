/// Query for the artifact content route. `inline` is honoured only for a
/// small, safe media whitelist; everything else is forced to a download.
#[derive(Debug, serde::Deserialize)]
pub struct ArtifactContentQuery {
    #[serde(default)]
    pub disposition: Option<String>,
}

fn artifact_content_disposition(
    file_name: &str,
    inline: bool,
) -> Result<HeaderValue, axum::http::header::InvalidHeaderValue> {
    let mut encoded = String::new();
    for byte in file_name.as_bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                *byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            encoded.push(*byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    let kind = if inline { "inline" } else { "attachment" };
    HeaderValue::from_str(&format!("{kind}; filename*=UTF-8''{encoded}"))
}

fn artifact_inline_allowed(media_type: &str) -> bool {
    matches!(
        media_type,
        "image/png" | "image/jpeg" | "image/webp" | "application/pdf"
    )
}

/// Stream one delivered file. Authentication follows the session; the path is
/// resolved from the stored delivery record, never from the request.
pub async fn download_artifact(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    Path((session_code, run_code, item_code)): Path<(String, String, String)>,
    Query(query): Query<ArtifactContentQuery>,
) -> Response {
    let content = match state
        .chats
        .artifact_content(&session_code, &run_code, &item_code)
    {
        Ok(content) => content,
        Err(error) => return chat_error_response(error),
    };
    let file = match std::fs::File::open(&content.path) {
        Ok(file) => file,
        Err(_) => return chat_error_response(ChatError::ArtifactNotFound),
    };
    let inline = query.disposition.as_deref() == Some("inline")
        && artifact_inline_allowed(&content.media_type);
    let media_type = content.media_type.clone();
    let byte_size = content.byte_size;
    let file_name = content.file_name.clone();
    let file = tokio::fs::File::from_std(file);
    let stream = async_stream::stream! {
        let mut file = file;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            match tokio::io::AsyncReadExt::read(&mut file, &mut buffer).await {
                Ok(0) => break,
                Ok(read) => {
                    yield Ok::<axum::body::Bytes, std::io::Error>(
                        axum::body::Bytes::copy_from_slice(&buffer[..read]),
                    );
                }
                Err(error) => {
                    yield Err(error);
                    break;
                }
            }
        }
    };
    let mut response = Response::new(Body::from_stream(stream));
    if let Ok(value) = HeaderValue::from_str(&media_type) {
        response.headers_mut().insert(CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&byte_size.to_string()) {
        response.headers_mut().insert(CONTENT_LENGTH, value);
    }
    if let Ok(value) = artifact_content_disposition(&file_name, inline) {
        response.headers_mut().insert(CONTENT_DISPOSITION, value);
    }
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDesktopPath {
    pub path: String,
}

/// Privileged resolution used only by the trusted desktop main process. The
/// renderer never receives this path: it forwards opaque identifiers and the
/// main process performs the request with the user's short-lived token.
pub async fn resolve_artifact(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    Path((session_code, run_code, item_code)): Path<(String, String, String)>,
) -> Response {
    match state
        .chats
        .artifact_content(&session_code, &run_code, &item_code)
    {
        Ok(content) => Json(ApiResponse::success(ArtifactDesktopPath {
            path: content.path.to_string_lossy().into_owned(),
        }))
        .into_response(),
        Err(error) => chat_error_response(error),
    }
}
