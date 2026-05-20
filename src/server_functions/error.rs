#[cfg(feature = "ssr")]
pub use ssr::{ctx, map_app_error, map_setup_error};

#[cfg(feature = "ssr")]
mod ssr {
    use std::any::type_name;

    use http::StatusCode;
    use leptos::prelude::*;
    use leptos_axum::ResponseOptions;

    use crate::server::application::AppError;
    use crate::server::setup::SetupError;
    use crate::shared::contracts::ApiError;

    pub fn ctx<T: Clone + 'static>() -> Result<T, ServerFnError> {
        use_context::<T>()
            .ok_or_else(|| ServerFnError::new(format!("missing context: {}", type_name::<T>())))
    }

    pub fn map_app_error(err: &AppError) -> ServerFnError {
        let api_error = ApiError::from(err);
        set_status(
            StatusCode::from_u16(api_error.http_status())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        );
        ServerFnError::new(api_error.message)
    }

    pub fn map_setup_error(err: &SetupError) -> ServerFnError {
        let status = match &err {
            SetupError::Config(_) => StatusCode::BAD_REQUEST,
            SetupError::MissingVariable(_) | SetupError::InvalidVariable(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            SetupError::Io(_) | SetupError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        set_status(status);
        ServerFnError::new(err.to_string())
    }

    fn set_status(status: StatusCode) {
        if let Some(opts) = use_context::<ResponseOptions>() {
            opts.set_status(status);
        }
    }
}
