use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;
use shared::dtos::LocationPointDto;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;

use crate::api;
use crate::components::{Loading, Message, ToastSignal};

mod map_interop;
mod outlier_panel;
use outlier_panel::OutlierPanel;

const MAP_CONTAINER_ID: &str = "locations-map";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeRange {
    LastWeek,
    LastMonth,
    AllTime,
}

impl TimeRange {
    const MS_PER_DAY: i64 = 24 * 60 * 60 * 1000;

    /// `since_epoch_ms` bound for this range as of "now" (client-side, so adding a
    /// future preset never touches the server) - `None` only for `AllTime`.
    fn since_epoch_ms(self) -> Option<i64> {
        let days = match self {
            TimeRange::LastWeek => 7,
            TimeRange::LastMonth => 30,
            TimeRange::AllTime => return None,
        };
        Some(js_sys::Date::now() as i64 - days * Self::MS_PER_DAY)
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TimeRange::LastWeek => "last-week",
            TimeRange::LastMonth => "last-month",
            TimeRange::AllTime => "all-time",
        })
    }
}

impl From<&str> for TimeRange {
    fn from(value: &str) -> Self {
        match value {
            "last-week" => TimeRange::LastWeek,
            "all-time" => TimeRange::AllTime,
            _ => TimeRange::LastMonth,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Dots,
    Line,
}

impl RenderMode {
    fn as_key(self) -> &'static str {
        match self {
            RenderMode::Dots => "dots",
            RenderMode::Line => "line",
        }
    }
}

impl From<&str> for RenderMode {
    fn from(value: &str) -> Self {
        match value {
            "line" => RenderMode::Line,
            _ => RenderMode::Dots,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapStyle {
    Streets,
    Satellite,
    Outdoor,
    Basic,
}

impl MapStyle {
    /// MapTiler's own style slug, as used in `https://api.maptiler.com/maps/{slug}/style.json`
    /// (see `map_interop.rs::styleFor` - we build style URLs by hand since
    /// `@maptiler/sdk`'s style-name constants aren't available without its ESM-only SDK).
    fn as_key(self) -> &'static str {
        match self {
            MapStyle::Streets => "streets-v2",
            MapStyle::Satellite => "satellite",
            MapStyle::Outdoor => "outdoor-v2",
            MapStyle::Basic => "basic-v2",
        }
    }
}

impl From<&str> for MapStyle {
    fn from(value: &str) -> Self {
        match value {
            "satellite" => MapStyle::Satellite,
            "outdoor-v2" => MapStyle::Outdoor,
            "basic-v2" => MapStyle::Basic,
            _ => MapStyle::Streets,
        }
    }
}

#[component]
pub fn LocationsPage() -> impl IntoView {
    let query = use_query_map();
    let initial_range = query
        .with_untracked(|q| q.get("range"))
        .as_deref()
        .map(TimeRange::from)
        .unwrap_or(TimeRange::LastMonth);

    let time_range = RwSignal::new(initial_range);
    let render_mode = RwSignal::new(RenderMode::Dots);
    let map_style = RwSignal::new(MapStyle::Streets);
    let msg = ToastSignal::new();
    let map_ready = RwSignal::new(false);
    let points: RwSignal<Vec<LocationPointDto>> = RwSignal::new(Vec::new());
    let selected_outlier_ids: RwSignal<HashSet<i64>> = RwSignal::new(HashSet::new());
    let current_position: RwSignal<Option<(f64, f64)>> = RwSignal::new(None);

    let points_resource = LocalResource::new(move || {
        let since = time_range.get().since_epoch_ms();
        api::fetch_location_points(since, None)
    });

    // A fresh range means a fresh point set - drop any outlier selection left over
    // from the previous one so stale ids can't linger into it.
    Effect::new(move |_| {
        let _ = time_range.get();
        selected_outlier_ids.update(|s| s.clear());
    });

    // Init the map once the container div exists in the DOM. The MapTiler key is
    // fetched from the server rather than baked into the wasm bundle (see
    // GET /api/map-config); a missing key still initializes the map (MapTiler will
    // just fail to load tiles), surfaced as a toast rather than silently.
    Effect::new(move |_| {
        request_animation_frame(move || {
            spawn_local(async move {
                let key = api::fetch_map_config()
                    .await
                    .ok()
                    .and_then(|c| c.maptiler_key)
                    .unwrap_or_default();
                if key.is_empty() {
                    msg.error(
                        "No MAPTILER_API_KEY configured on the server - map tiles won't load.",
                    );
                }
                map_interop::init_map(MAP_CONTAINER_ID, &key, map_style.get_untracked().as_key());
                map_ready.set(true);
            });
        });
    });

    // Push freshly (re)loaded points into the map and fit the view to them. Depends
    // only on `map_ready`/`points` (render_mode read untracked) so toggling the
    // render mode elsewhere never re-triggers a bounds jump.
    Effect::new(move |_| {
        if !map_ready.get() {
            return;
        }
        let pts = points.get();
        let json = serde_json::to_string(&pts).unwrap_or_default();
        map_interop::render_points(&json, render_mode.get_untracked().as_key());
    });

    // Continuously watch the browser's position while this page is mounted, torn
    // down on unmount - mirrors the live log tail's EventSource lifecycle in
    // `pages/logs.rs`. Requires a secure context (HTTPS or localhost); that failure
    // (and any other) surfaces as a toast rather than leaving "Jump to me" silently
    // disabled with no explanation.
    Effect::new(move |_| {
        let on_update = Closure::<dyn FnMut(f64, f64)>::new(move |lat: f64, lon: f64| {
            current_position.set(Some((lat, lon)));
            map_interop::render_current_position(lat, lon);
        });
        let on_error = Closure::<dyn FnMut(String)>::new(move |message: String| {
            msg.error(format!("Location unavailable: {message}"));
        });

        let watch_id: JsValue = map_interop::start_watching_position(&on_update, &on_error);
        on_update.forget();
        on_error.forget();

        on_cleanup(move || {
            map_interop::stop_watching_position(watch_id);
        });
    });

    view! {
        <div class="container">
            <h1>"Locations"</h1>
            <Message signal=msg />

            <div class="flex gap-2 locations-toolbar">
                <select
                    class="btn btn-secondary"
                    prop:value=move || time_range.get().to_string()
                    on:change=move |ev| {
                        time_range.set(TimeRange::from(event_target_value(&ev).as_str()));
                    }
                >
                    <option value="last-week">"Last week"</option>
                    <option value="last-month">"Last month"</option>
                    <option value="all-time">"All time"</option>
                </select>

                <select
                    class="btn btn-secondary"
                    prop:value=move || render_mode.get().as_key()
                    on:change=move |ev| {
                        let mode = RenderMode::from(event_target_value(&ev).as_str());
                        render_mode.set(mode);
                        if map_ready.get_untracked() {
                            map_interop::set_render_mode(mode.as_key());
                        }
                    }
                >
                    <option value="dots">"Dots"</option>
                    <option value="line">"Route line"</option>
                </select>

                <select
                    class="btn btn-secondary"
                    prop:value=move || map_style.get().as_key()
                    on:change=move |ev| {
                        let style = MapStyle::from(event_target_value(&ev).as_str());
                        map_style.set(style);
                        if map_ready.get_untracked() {
                            map_interop::set_map_style(style.as_key());
                        }
                    }
                >
                    <option value="streets-v2">"Streets"</option>
                    <option value="satellite">"Satellite"</option>
                    <option value="outdoor-v2">"Outdoor"</option>
                    <option value="basic-v2">"Basic"</option>
                </select>

                <button
                    class="btn btn-secondary"
                    disabled=move || current_position.get().is_none()
                    on:click=move |_| map_interop::fly_to_current_position()
                >
                    "Jump to me"
                </button>
            </div>

            <div id=MAP_CONTAINER_ID class="locations-map"></div>

            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match points_resource.await {
                        Ok(list) => {
                            points.set(list);
                            view! { <OutlierPanel points selected_outlier_ids msg /> }.into_any()
                        }
                        Err(e) => {
                            view! { <div class="message message-error">"Error: " {e}</div> }
                                .into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}
