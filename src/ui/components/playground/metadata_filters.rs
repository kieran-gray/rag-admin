use leptos::prelude::*;

use crate::shared::contracts::MetadataFilterDto;

const DEFAULT_FIELD: &str = "source_slug";
const FIELD_OPTIONS: &[&str] = &[DEFAULT_FIELD, "source_version"];

pub fn metadata_filters_from_signal(
    signal: RwSignal<Vec<MetadataFilterDto>>,
) -> Vec<MetadataFilterDto> {
    signal
        .get_untracked()
        .into_iter()
        .filter_map(|f| {
            let field = f.field.trim().to_string();
            let value = f.value.trim().to_string();
            if field.is_empty() || value.is_empty() {
                None
            } else {
                Some(MetadataFilterDto { field, value })
            }
        })
        .collect()
}

#[component]
pub fn MetadataFilters(filters: RwSignal<Vec<MetadataFilterDto>>) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (draft_field, set_draft_field) = signal::<String>(DEFAULT_FIELD.to_string());
    let (draft_value, set_draft_value) = signal(String::new());

    let commit = move || {
        let field = draft_field.get_untracked();
        let value = draft_value.get_untracked();
        let field_trimmed = field.trim();
        let value_trimmed = value.trim();
        if field_trimmed.is_empty() || value_trimmed.is_empty() {
            return;
        }
        let new_entry = MetadataFilterDto {
            field: field_trimmed.to_string(),
            value: value_trimmed.to_string(),
        };
        filters.update(|list| {
            list.retain(|f| f.field != new_entry.field);
            list.push(new_entry);
        });
        set_draft_value.set(String::new());
        set_editing.set(false);
    };

    let remove = move |field: String| {
        filters.update(|list| list.retain(|f| f.field != field));
    };

    view! {
        <div class="metadata-filters">
            {move || {
                let list = filters.get();
                list.into_iter().map(|f| {
                    let field_for_remove = f.field.clone();
                    let label = format!("{}={}", f.field, f.value);
                    view! {
                        <button
                            type="button"
                            class="metadata-filter-chip"
                            on:click=move |_| remove(field_for_remove.clone())
                            title="Remove filter"
                        >
                            <span>{label}</span>
                            <span class="metadata-filter-chip-remove" aria-hidden="true">"×"</span>
                        </button>
                    }
                }).collect_view()
            }}

            {move || {
                if editing.get() {
                    view! {
                        <div class="metadata-filter-editor">
                            <select
                                class="metadata-filter-field"
                                on:change=move |ev| set_draft_field.set(event_target_value(&ev))
                            >
                                {FIELD_OPTIONS.iter().map(|opt| {
                                    let opt = *opt;
                                    view! {
                                        <option
                                            value=opt
                                            selected=move || draft_field.get() == opt
                                        >
                                            {opt}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                            <input
                                type="text"
                                class="metadata-filter-value"
                                placeholder="value"
                                prop:value=move || draft_value.get()
                                on:input=move |ev| set_draft_value.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    match ev.key().as_str() {
                                        "Enter" => { ev.prevent_default(); commit(); }
                                        "Escape" => {
                                            ev.prevent_default();
                                            set_draft_value.set(String::new());
                                            set_editing.set(false);
                                        }
                                        _ => {}
                                    }
                                }
                                autofocus
                            />
                            <button type="button" class="btn btn-sm btn-primary" on:click=move |_| commit()>
                                "Add"
                            </button>
                            <button
                                type="button"
                                class="btn btn-sm btn-ghost"
                                on:click=move |_| {
                                    set_draft_value.set(String::new());
                                    set_editing.set(false);
                                }
                            >
                                "Cancel"
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <button
                            type="button"
                            class="metadata-filter-add"
                            on:click=move |_| set_editing.set(true)
                        >
                            "+ Add filter"
                        </button>
                    }.into_any()
                }
            }}
        </div>
    }
}
