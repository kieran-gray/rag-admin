use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Pending,
    Fail,
    Stale,
    Info,
    Neutral,
}

impl Status {
    fn class(self) -> &'static str {
        match self {
            Self::Ok => "pill pill-ok",
            Self::Pending => "pill pill-pending",
            Self::Fail => "pill pill-fail",
            Self::Stale => "pill pill-stale",
            Self::Info => "pill pill-info",
            Self::Neutral => "pill pill-neutral",
        }
    }
}

#[component]
pub fn StatusPill(
    #[prop(into)] label: String,

    #[prop(optional, into)] kind: Option<Status>,
) -> impl IntoView {
    let kind = kind.unwrap_or(Status::Neutral);
    view! { <span class=kind.class()>{label}</span> }
}
