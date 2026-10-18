use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::components::MenuIcon;
use crate::sse::{ConnectionStatus, ConnectionStatusSignal};

#[component]
pub fn Navbar() -> impl IntoView {
    let open = RwSignal::new(false);
    let location = use_location();
    // collapse the drawer after every navigation
    Effect::new(move |_| {
        location.pathname.track();
        open.set(false);
    });

    view! {
        <nav class="navbar" class:open=open>
            <A href="/app/" attr:class="brand">"File Sync - Admin"</A>
            <div class="nav-links">
                <A href="/app/clients">"Clients"</A>
                <A href="/app/watch-groups">"Watch Groups"</A>
                <A href="/app/links">"Links"</A>
                <A href="/app/locations">"Locations"</A>
                <A href="/app/monitor">"Monitor"</A>
                <A href="/app/backups">"Backups"</A>
                <A href="/app/logs">"Logs"</A>
            </div>
            <ConnectionIndicator />
            <button
                class="nav-toggle"
                aria-label="Menu"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <MenuIcon />
            </button>
        </nav>
    }
}

/// Small status dot reflecting the app-wide SSE connection: grey while connecting,
/// green once live, red if it failed or dropped.
#[component]
fn ConnectionIndicator() -> impl IntoView {
    let status = use_context::<ConnectionStatusSignal>()
        .expect("ConnectionStatusSignal not provided")
        .0;

    let class = move || match status.get() {
        ConnectionStatus::Connecting => "status-dot status-dot--connecting",
        ConnectionStatus::Connected => "status-dot status-dot--connected",
        ConnectionStatus::Failed => "status-dot status-dot--failed",
    };
    let title = move || match status.get() {
        ConnectionStatus::Connecting => "Connecting to server...",
        ConnectionStatus::Connected => "Live updates connected",
        ConnectionStatus::Failed => "Live updates disconnected",
    };

    view! { <span class=class title=title></span> }
}
