use crate::server::application::ports::Clock;
use crate::server::domain::shared::Timestamp;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        use time::format_description::well_known::Rfc3339;
        use time::OffsetDateTime;
        Timestamp::new(
            OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        )
    }
}
