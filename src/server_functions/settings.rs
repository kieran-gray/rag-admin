use leptos::prelude::*;

use crate::shared::SettingsDto;

#[cfg(feature = "ssr")]
use crate::server::application::configuration::ports::EvaluationDefaultsStore;
#[cfg(feature = "ssr")]
use crate::server_functions::error::ctx;
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = LoadSettings, prefix = "/api", endpoint = "load_settings")]
pub async fn load_settings() -> Result<SettingsDto, ServerFnError> {
    ctx::<Arc<dyn EvaluationDefaultsStore>>()?
        .load()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = SaveSettings, prefix = "/api", endpoint = "save_settings")]
pub async fn save_settings(settings: SettingsDto) -> Result<(), ServerFnError> {
    ctx::<Arc<dyn EvaluationDefaultsStore>>()?
        .save(settings)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}
