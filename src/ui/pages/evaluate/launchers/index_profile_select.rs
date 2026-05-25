use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::IndexProfileDto;

#[component]
pub fn IndexProfileSelect(
    index_profiles: StoredValue<Vec<IndexProfileDto>>,
    value: ReadSignal<Option<Uuid>>,
    set_value: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3 flex-wrap">
            <span class="eyebrow shrink-0">"Index profile"</span>
            <select
                class="input max-w-md"
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    if v.is_empty() {
                        set_value.set(None);
                    } else if let Ok(id) = v.parse::<Uuid>() {
                        set_value.set(Some(id));
                    }
                }
            >
                <option value="">"— select index profile —"</option>
                {move || {
                    let ps = index_profiles.get_value();
                    ps.into_iter().map(|pc| {
                        let id = pc.index_profile_id;
                        let selected = value.get() == Some(id);
                        view! {
                            <option value=id.to_string() selected=selected>
                                {pc.name}
                            </option>
                        }
                    }).collect_view()
                }}
            </select>
            <span class="text-xs faint">"Applies to all launchers below."</span>
        </div>
    }
}
