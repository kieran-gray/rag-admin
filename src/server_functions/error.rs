#[cfg(feature = "ssr")]
pub use ssr::{ctx, map_app_error, map_setup_error};

#[cfg(feature = "ssr")]
mod ssr {
    use http::StatusCode;
    use leptos::prelude::*;
    use leptos_axum::ResponseOptions;

    use crate::server::application::AppError;
    use crate::server::setup::SetupError;

    pub fn ctx<T: Clone + 'static>() -> Result<T, ServerFnError> {
        use_context::<T>().ok_or_else(|| {
            ServerFnError::new(format!("missing context: {}", std::any::type_name::<T>()))
        })
    }

    pub fn map_app_error(err: AppError) -> ServerFnError {
        let status = match &err {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::Io(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        set_status(status);
        ServerFnError::new(err.to_string())
    }

    pub fn map_setup_error(err: SetupError) -> ServerFnError {
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
