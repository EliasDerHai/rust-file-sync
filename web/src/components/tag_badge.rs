use leptos::prelude::*;

#[component]
pub fn TagBadge(
    tag: String,
    #[prop(optional, into)] active: Signal<bool>,
    #[prop(optional)] on_click: Option<Callback<()>>,
) -> impl IntoView {
    let class = move || match (on_click.is_some(), active.get()) {
        (true, true) => "tag clickable active",
        (true, false) => "tag clickable",
        _ => "tag",
    };
    view! {
        <span class=class on:click=move |_| { if let Some(cb) = on_click { cb.run(()); } }>
            {tag}
        </span>
    }
}
