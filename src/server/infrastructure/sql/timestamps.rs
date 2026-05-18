use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::server::application::AppError;
use crate::server::domain::shared::Timestamp;

pub fn to_offset_datetime(ts: &Timestamp) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(ts.as_str(), &Rfc3339)
        .map_err(|e| AppError::Internal(format!("parse timestamp '{}': {e}", ts.as_str())))
}

pub fn from_offset_datetime(odt: OffsetDateTime) -> Result<Timestamp, AppError> {
    odt.format(&Rfc3339)
        .map(Timestamp::from)
        .map_err(|e| AppError::Internal(format!("format timestamp: {e}")))
}
