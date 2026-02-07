use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

mod api;
mod components;
mod pages;
mod types;

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    view! {
        <Router base="/app">
            <components::Navbar />
            <main>
                <Routes fallback=|| view! { <div class="container"><h1>"Page not found"</h1></div> }>
                    <Route path=path!("/") view=pages::ConfigsPage />
                    <Route path=path!("/configs") view=pages::ConfigsPage />
                    <Route path=path!("/config/:id") view=pages::ConfigEditPage />
                    <Route path=path!("/watch-groups") view=pages::WatchGroupsPage />
                    <Route path=path!("/monitor") view=pages::MonitorPage />
                </Routes>
            </main>
        </Router>
    }
}
