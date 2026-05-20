use leptos::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationSummary {
    pub shown: usize,
    pub total_matching: u64,
    pub total_all: u64,
    pub has_next: bool,
}

#[derive(Clone, Copy)]
pub struct PaginationControls {
    pub current_cursor: Signal<Option<String>>,
    pub page_number: Signal<usize>,
    pub can_go_prev: Signal<bool>,
    pub on_prev: Callback<()>,
    pub push_cursor: Callback<String>,
    pub reset: Callback<()>,
}

pub fn use_cursor_pagination() -> PaginationControls {
    let (cursor_stack, set_cursor_stack) = signal::<Vec<Option<String>>>(vec![None]);

    let current_cursor = Signal::derive(move || cursor_stack.with(|s| s.last().cloned().flatten()));
    let page_number = Signal::derive(move || cursor_stack.with(Vec::len));
    let can_go_prev = Signal::derive(move || cursor_stack.with(|s| s.len() > 1));

    let on_prev = Callback::new(move |_| {
        set_cursor_stack.update(|stack| {
            if stack.len() > 1 {
                stack.pop();
            }
        });
    });

    let push_cursor = Callback::new(move |next: String| {
        set_cursor_stack.update(|stack| stack.push(Some(next)));
    });

    let reset = Callback::new(move |_| {
        set_cursor_stack.set(vec![None]);
    });

    PaginationControls {
        current_cursor,
        page_number,
        can_go_prev,
        on_prev,
        push_cursor,
        reset,
    }
}

#[component]
pub fn PaginationBar(
    #[prop(into)] summary: Signal<Option<PaginationSummary>>,
    controls: PaginationControls,
    on_next: Callback<()>,
) -> impl IntoView {
    let on_prev = controls.on_prev;
    let can_go_prev = controls.can_go_prev;
    let page_number = controls.page_number;

    view! {
        <div class="list-page-pagination">
            {move || summary.get().map(|p| {
                let extra = if p.total_matching == p.total_all {
                    format!("{} shown · {} total", p.shown, p.total_all)
                } else {
                    format!(
                        "{} shown · {} of {} match filters",
                        p.shown, p.total_matching, p.total_all
                    )
                };
                let has_next = p.has_next;
                view! {
                    <>
                        <span>{extra}</span>
                        <span class="list-page-pagination-spacer"></span>
                        <span class="list-page-indicator">
                            {move || format!("Page {}", page_number.get())}
                        </span>
                        <button
                            type="button"
                            class="btn"
                            prop:disabled=move || !can_go_prev.get()
                            on:click=move |_| on_prev.run(())
                        >
                            "‹ Prev"
                        </button>
                        <button
                            type="button"
                            class="btn"
                            prop:disabled=!has_next
                            on:click=move |_| on_next.run(())
                        >
                            "Next ›"
                        </button>
                    </>
                }
            })}
        </div>
    }
}
