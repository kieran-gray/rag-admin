use leptos::prelude::*;

use crate::ui::components::primitives::Surface;

#[derive(Clone, Copy)]
pub(crate) struct PromoteHandle {
    pub run_id: uuid::Uuid,
    pub busy_label: ReadSignal<Option<String>>,
    pub set_busy_label: WriteSignal<Option<String>>,
    pub status: ReadSignal<Option<Result<String, String>>>,
    pub set_status: WriteSignal<Option<Result<String, String>>>,
}

impl PromoteHandle {
    pub fn new(run_id: uuid::Uuid) -> Self {
        let (busy_label, set_busy_label) = signal::<Option<String>>(None);
        let (status, set_status) = signal::<Option<Result<String, String>>>(None);
        Self {
            run_id,
            busy_label,
            set_busy_label,
            status,
            set_status,
        }
    }

    pub fn save(self, variant_label: String) {
        use crate::server_functions::evaluation::promote_variant_to_chunking_config;
        let run_id = self.run_id;
        let label_for_busy = variant_label.clone();
        let label_for_message = variant_label.clone();
        self.set_busy_label.set(Some(label_for_busy));
        self.set_status.set(None);
        leptos::task::spawn_local(async move {
            let res =
                promote_variant_to_chunking_config(run_id, variant_label, String::new()).await;
            self.set_busy_label.set(None);
            match res {
                Ok(_) => self.set_status.set(Some(Ok(format!(
                    "Saved '{label_for_message}'. Find it under Chunking."
                )))),
                Err(e) => self.set_status.set(Some(Err(e.to_string()))),
            }
        });
    }
}

#[component]
pub(crate) fn VariantSaveButton(
    variant_label: String,
    is_leader: bool,
    promote: PromoteHandle,
    #[prop(optional)] compact: bool,
) -> impl IntoView {
    let label_for_save = variant_label.clone();
    let label_for_busy = variant_label;
    let is_busy = move || {
        promote
            .busy_label
            .with(|b| b.as_deref() == Some(label_for_busy.as_str()))
    };
    let any_busy = move || promote.busy_label.with(Option::is_some);
    let class_base = if is_leader { "btn btn-primary" } else { "btn" };
    let class = if compact {
        format!("{class_base} btn-compact")
    } else {
        class_base.to_string()
    };
    let title = if is_leader {
        "Save this champion as a named chunking configuration."
    } else {
        "Save this variant's chunking config under Chunking."
    };
    view! {
        <button
            type="button"
            class=class
            title=title
            prop:disabled=any_busy
            on:click=move |_| promote.save(label_for_save.clone())
        >
            {move || if is_busy() { "Saving…" } else { "Save" }}
        </button>
    }
}

#[component]
pub(crate) fn ReplicatePanel(run_id: uuid::Uuid) -> impl IntoView {
    use crate::server_functions::evaluation::replicate_optimization_run;
    let (replicating, set_replicating) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let on_replicate = move |_| {
        set_error.set(None);
        set_replicating.set(true);
        leptos::task::spawn_local(async move {
            match replicate_optimization_run(run_id).await {
                Ok(new_run_id) => {
                    let url = format!("/runs/{new_run_id}?tab=compare&with={run_id}");
                    let _ =
                        leptos::web_sys::window().and_then(|w| w.location().set_href(&url).ok());
                }
                Err(e) => {
                    set_replicating.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    };

    view! {
        <Surface title="Sanity-check the champion".to_string()>
            <p class="text-xs muted mb-3">
                "Launch a sibling run with a fresh random seed. A matching champion is a noisy positive signal, not a formal confidence test. To save a chunking config, use the Save button on any row of the Variants table below."
            </p>
            <button
                type="button"
                class="btn"
                prop:disabled=move || replicating.get()
                on:click=on_replicate
            >
                {move || if replicating.get() { "Replicating…" } else { "Replicate (new seed)" }}
            </button>
            {move || error.get().map(|e| view! {
                <div class="log-line-error text-xs mt-2">{e}</div>
            })}
        </Surface>
    }
}
