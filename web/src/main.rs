use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

mod api;
mod components;
mod format;
mod pages;
mod sse;

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    provide_context(sse::init());

    view! {
        <Router base="/app">
            <components::Navbar />
            <main>
                <Routes fallback=|| view! { <div class="container"><h1>"Page not found"</h1></div> }>
                    <Route path=path!("/") view=pages::ClientsPage />
                    <Route path=path!("/clients") view=pages::ClientsPage />
                    <Route path=path!("/watch-groups") view=pages::WatchGroupsPage />
                    <Route path=path!("/watch-groups/:id") view=pages::WatchGroupFilesPage />
                    <Route path=path!("/watch-groups/:id/gallery") view=pages::MediaGalleryPage />
                    <Route path=path!("/links") view=pages::LinksPage />
                    <Route path=path!("/monitor") view=pages::MonitorPage />
                    <Route path=path!("/backups") view=pages::BackupsPage />
                    <Route path=path!("/logs") view=pages::LogsPage />
                </Routes>
            </main>
        </Router>
    }
}
