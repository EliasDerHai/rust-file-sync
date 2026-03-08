use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Navbar() -> impl IntoView {
    view! {
        <nav class="navbar">
            <A href="/app/" attr:class="brand">"File Sync - Admin"</A>
            <A href="/app/clients">"Clients"</A>
            <A href="/app/watch-groups">"Watch Groups"</A>
            <A href="/app/monitor">"Monitor"</A>
        </nav>
    }
}
