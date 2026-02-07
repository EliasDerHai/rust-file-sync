use leptos::prelude::*;

#[component]
pub fn Card(
    #[prop(optional)] dashed: bool,
    children: Children,
) -> impl IntoView {
    let class = if dashed { "card-dashed" } else { "card" };
    view! {
        <div class=class>
            {children()}
        </div>
    }
}
