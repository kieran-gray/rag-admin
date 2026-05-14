use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::configuration::{
    apply_chunking_configuration_command, apply_embedding_model_command,
    apply_generation_model_command, apply_pipeline_configuration_command,
    apply_sweep_template_command, apply_vector_index_command,
};
use crate::shared::{
    ChunkingConfigurationCommandDto, EmbeddingModelCommandDto, GenerationModelCommandDto,
    PipelineConfigurationCommandDto, SweepTemplateCommandDto, VectorIndexCommandDto,
};

pub fn parse_uuid_or_none(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value).ok()
}

pub fn short_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

pub fn run_embedding_model_command<F>(
    command: EmbeddingModelCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move { apply_embedding_model_command(command).await.map(|_| ()) },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

pub fn run_generation_model_command<F>(
    command: GenerationModelCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move { apply_generation_model_command(command).await.map(|_| ()) },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

pub fn run_vector_index_command<F>(
    command: VectorIndexCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move { apply_vector_index_command(command).await.map(|_| ()) },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

pub fn run_pipeline_configuration_command<F>(
    command: PipelineConfigurationCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move {
            apply_pipeline_configuration_command(command)
                .await
                .map(|_| ())
        },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

pub fn run_chunking_configuration_command<F>(
    command: ChunkingConfigurationCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move {
            apply_chunking_configuration_command(command)
                .await
                .map(|_| ())
        },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

pub fn run_sweep_template_command<F>(
    command: SweepTemplateCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    run_command(
        async move { apply_sweep_template_command(command).await.map(|_| ()) },
        success_message,
        set_busy,
        set_status,
        dialog_status,
        set_refresh,
        on_success,
    );
}

fn run_command<Fut, F>(
    future: Fut,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    Fut: std::future::Future<Output = Result<(), ServerFnError>> + 'static,
    F: FnOnce() + 'static,
{
    set_busy.set(true);
    set_status.set(None);
    if let Some(ds) = dialog_status {
        ds.set(None);
    }
    spawn_local(async move {
        match future.await {
            Ok(()) => {
                if let Some(ds) = dialog_status {
                    ds.set(None);
                }
                on_success();
                set_status.set(Some((true, success_message.to_string())));
                set_refresh.update(|v| *v += 1);
            }
            Err(e) => {
                let message = format!("COMMAND_FAULT: {e}");
                if let Some(ds) = dialog_status {
                    ds.set(Some(message));
                } else {
                    set_status.set(Some((false, message)));
                }
            }
        }
        set_busy.set(false);
    });
}
