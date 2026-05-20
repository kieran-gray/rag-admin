use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;

use crate::server::application::source_document::SourceDocumentIngestService;
use crate::server::application::AppError;
use crate::shared::contracts::ApiError;
use crate::shared::contracts::SourceDocumentDto;

pub async fn upload_document(
    Extension(ingest): Extension<Arc<SourceDocumentIngestService>>,
    mut multipart: Multipart,
) -> Result<Json<SourceDocumentDto>, UploadError> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| UploadError::bad_request(format!("multipart read failed: {e}")))?
    {
        if field.name() == Some("file") {
            filename = field.file_name().map(ToString::to_string);
            let data = field
                .bytes()
                .await
                .map_err(|e| UploadError::bad_request(format!("read file bytes: {e}")))?;
            bytes = Some(data.to_vec());
            break;
        }
    }

    let bytes = bytes.ok_or_else(|| UploadError::bad_request("missing `file` part"))?;
    let filename =
        filename.ok_or_else(|| UploadError::bad_request("multipart file is missing a filename"))?;

    let dto = ingest
        .import_upload(bytes, filename)
        .await
        .map_err(UploadError::from)?;
    Ok(Json(dto))
}

pub struct UploadError {
    status: StatusCode,
    message: String,
}

impl UploadError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
}

impl From<AppError> for UploadError {
    fn from(err: AppError) -> Self {
        let api_error = ApiError::from(err);
        let status = StatusCode::from_u16(api_error.http_status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        Self {
            status,
            message: api_error.message,
        }
    }
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}
