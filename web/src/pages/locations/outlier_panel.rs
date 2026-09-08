use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::LocationPointDto;

use crate::api;
use crate::components::{ConfirmDialog, ToastSignal};

/// Review list of heuristically-flagged points (see
/// `LocationPointRepository::get_range` for the accuracy/implied-speed thresholds),
/// with bulk soft-delete ("exclude") once the user confirms which ones are actually
/// outliers.
#[component]
pub(super) fn OutlierPanel(
    points: RwSignal<Vec<LocationPointDto>>,
    selected_outlier_ids: RwSignal<HashSet<i64>>,
    msg: ToastSignal,
) -> impl IntoView {
    let show_confirm = RwSignal::new(false);

    let flagged = move || {
        points
            .get()
            .into_iter()
            .filter(|p| p.is_flagged)
            .collect::<Vec<_>>()
    };

    let select_all = move |_| {
        let ids: HashSet<i64> = flagged().iter().map(|p| p.id).collect();
        selected_outlier_ids.set(ids);
    };

    let on_confirm_exclude = move || {
        let ids: Vec<i64> = selected_outlier_ids.get_untracked().into_iter().collect();
        if ids.is_empty() {
            return;
        }
        spawn_local(async move {
            match api::soft_delete_location_points(&ids).await {
                Ok(count) => {
                    points.update(|v| v.retain(|p| !ids.contains(&p.id)));
                    selected_outlier_ids.update(|s| s.clear());
                    msg.success(format!("Excluded {count} point(s)."));
                }
                Err(e) => msg.error(format!("Exclude failed: {e}")),
            }
        });
    };

    view! {
        <div class="card">
            <div class="flex-between">
                <h3>"Flagged points (" {move || flagged().len()} ")"</h3>
                <div class="flex gap-1">
                    <button class="btn btn-secondary" on:click=select_all>
                        "Select all"
                    </button>
                    <Show when=move || !selected_outlier_ids.get().is_empty()>
                        <button
                            class="btn btn-danger"
                            on:click=move |_| show_confirm.set(true)
                        >
                            "Exclude selected (" {move || selected_outlier_ids.get().len()} ")"
                        </button>
                    </Show>
                </div>
            </div>
            <Show when=move || flagged().is_empty()>
                <p class="text-muted text-sm">"No flagged points in this range."</p>
            </Show>
            <div class="outlier-list">
                {move || {
                    flagged()
                        .into_iter()
                        .map(|p| {
                            let id = p.id;
                            let is_selected = move || selected_outlier_ids.get().contains(&id);
                            let on_toggle = move |_| {
                                selected_outlier_ids.update(|s| {
                                    if s.contains(&id) {
                                        s.remove(&id);
                                    } else {
                                        s.insert(id);
                                    }
                                });
                            };
                            let accuracy = p
                                .accuracy_meters
                                .map(|a| format!("±{a:.0}m accuracy"))
                                .unwrap_or_else(|| "accuracy unknown".to_string());
                            let speed = p
                                .speed_meters_per_second
                                .map(|s| format!(", {s:.1} m/s recorded"))
                                .unwrap_or_default();
                            view! {
                                <label class="outlier-row checkbox-group">
                                    <input
                                        type="checkbox"
                                        prop:checked=is_selected
                                        on:change=on_toggle
                                    />
                                    <span class="text-mono text-sm">
                                        {p.timestamp_epoch_ms.to_string()}
                                    </span>
                                    <span class="text-muted text-sm">{accuracy}{speed}</span>
                                </label>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <ConfirmDialog
                show=show_confirm
                message="Exclude the selected point(s)? They'll be hidden from the map but kept in the database - safe to review again later.".to_string()
                on_confirm=on_confirm_exclude
            />
        </div>
    }
}
