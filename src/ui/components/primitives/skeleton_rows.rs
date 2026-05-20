use leptos::prelude::*;

#[derive(Debug, Clone)]
pub struct SkeletonColumn {
    pub width: &'static str,
    pub sub_width: Option<&'static str>,
    pub align_right: bool,
}

impl SkeletonColumn {
    pub fn new(width: &'static str) -> Self {
        Self {
            width,
            sub_width: None,
            align_right: false,
        }
    }

    pub fn with_sub(width: &'static str, sub_width: &'static str) -> Self {
        Self {
            width,
            sub_width: Some(sub_width),
            align_right: false,
        }
    }

    pub fn right(width: &'static str) -> Self {
        Self {
            width,
            sub_width: None,
            align_right: true,
        }
    }

    pub fn empty() -> Self {
        Self {
            width: "",
            sub_width: None,
            align_right: false,
        }
    }
}

#[component]
pub fn SkeletonRows(
    columns: Vec<SkeletonColumn>,
    #[prop(default = 6)] rows: usize,
) -> impl IntoView {
    view! {
        {(0..rows).map(move |_| {
            let cells = columns.iter().map(render_cell).collect_view();
            view! { <tr class="list-page-skeleton-row">{cells}</tr> }
        }).collect_view()}
    }
}

fn render_cell(column: &SkeletonColumn) -> impl IntoView {
    let width = column.width;
    let sub_width = column.sub_width;
    let align_right = column.align_right;

    if width.is_empty() {
        return view! { <td></td> }.into_any();
    }

    let bar_style = if align_right {
        format!("width: {width}; display:inline-block")
    } else {
        format!("width: {width}")
    };
    let td_class = if align_right { "text-right" } else { "" };

    view! {
        <td class=td_class>
            <div class="list-page-skeleton-bar" style=bar_style></div>
            {sub_width.map(|w| view! {
                <div
                    class="list-page-skeleton-bar"
                    style=format!("width: {w}; margin-top: 0.3rem; height: 0.5rem")
                ></div>
            })}
        </td>
    }
    .into_any()
}
