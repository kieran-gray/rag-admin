use leptos::prelude::*;

use crate::shared::contracts::SourceDescriptorDto;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn SourceMark(
    source: SourceDescriptorDto,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    let (class_suffix, glyph, label) = match &source {
        SourceDescriptorDto::Upload => ("upload", "↥", "Upload".to_string()),
        SourceDescriptorDto::Url { host, .. } => ("url", "@", host.clone()),
    };
    let badge_class = format!("docs-source-badge docs-source-badge-{class_suffix}");
    let title_attr = match &source {
        SourceDescriptorDto::Upload => "Manual upload".to_string(),
        SourceDescriptorDto::Url { url, .. } => url.clone(),
    };

    if compact {
        view! {
            <span class="docs-source-badge-icon" title=title_attr.clone()>{glyph}</span>
        }
        .into_any()
    } else {
        view! {
            <span class=badge_class title=title_attr>
                <span class="docs-source-badge-icon">{glyph}</span>
                <span class="docs-source-badge-label">{label}</span>
            </span>
        }
        .into_any()
    }
}
