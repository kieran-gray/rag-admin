use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::RetrievalProfileDto;

#[component]
pub fn RetrievalProfilePicker(
    profiles: Vec<RetrievalProfileDto>,
    value: ReadSignal<Uuid>,
    set_value: WriteSignal<Uuid>,
) -> impl IntoView {
    view! {
        <label class="playground-picker-label">
            <span class="eyebrow">"Retrieval profile"</span>
            <select
                class="playground-profile-picker"
                on:change=move |ev| {
                    if let Ok(uuid) = Uuid::parse_str(&event_target_value(&ev)) {
                        set_value.set(uuid);
                    }
                }
            >
                {profiles.into_iter().map(|p| {
                    let pid = p.retrieval_profile_id;
                    let id_str = pid.to_string();
                    view! {
                        <option value=id_str.clone() selected=move || value.get() == pid>
                            {p.name}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}
