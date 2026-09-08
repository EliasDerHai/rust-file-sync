use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

// MapLibre GL JS (loaded as the `maplibregl` UMD global via a <script> tag in
// index.html, same as Chart.js) + `navigator.geolocation`, wrapped the same way
// `monitor.rs` wraps Chart.js: a `#[wasm_bindgen(inline_js = ...)]` block of named
// JS functions, called from Rust by passing JSON strings / primitives across the
// boundary rather than binding the JS types themselves.
//
// `@maptiler/sdk` ships ESM-only (no UMD build, so it can't be a plain <script>
// tag like the rest of this codebase's CDN dependencies) - it's a thin wrapper
// around MapLibre GL JS anyway, so we drive MapLibre directly and build MapTiler's
// own style JSON URLs by hand instead of using the SDK's style-name convenience
// constants.
#[wasm_bindgen(inline_js = r#"
let _map = null;
let _loaded = false;
let _apiKey = '';
let _pending = { points: [], mode: 'dots', currentPos: null };

const MAX_RENDERED_POINTS = 20000;

function emptyFC() {
    return { type: 'FeatureCollection', features: [] };
}

// `styleKey` is one of MapTiler's own style slugs (e.g. "streets-v2", "satellite",
// "outdoor-v2", "basic-v2") - see `MapStyle::as_key` in mod.rs.
function styleFor(styleKey) {
    return `https://api.maptiler.com/maps/${styleKey}/style.json?key=${_apiKey}`;
}

function toPointsFeatureCollection(points) {
    return {
        type: 'FeatureCollection',
        features: points.map(p => ({
            type: 'Feature',
            geometry: { type: 'Point', coordinates: [p.longitude, p.latitude] },
            properties: { flagged: !!p.is_flagged },
        })),
    };
}

// A time-ordered line through the non-flagged points, so a single GPS jump doesn't
// draw a visible spike across the whole trace - flagged points still show as dots.
function toLineFeatureCollection(points) {
    const coords = points
        .filter(p => !p.is_flagged)
        .slice()
        .sort((a, b) => a.timestamp_epoch_ms - b.timestamp_epoch_ms)
        .map(p => [p.longitude, p.latitude]);
    if (coords.length < 2) return emptyFC();
    return {
        type: 'FeatureCollection',
        features: [{ type: 'Feature', geometry: { type: 'LineString', coordinates: coords }, properties: {} }],
    };
}

function applyData() {
    if (!_loaded) return;
    _map.getSource('location-points').setData(toPointsFeatureCollection(_pending.points));
    _map.getSource('location-line').setData(toLineFeatureCollection(_pending.points));
}

function applyVisibility() {
    if (!_loaded) return;
    const showLine = _pending.mode === 'line';
    _map.setLayoutProperty('route-line', 'visibility', showLine ? 'visible' : 'none');
    _map.setLayoutProperty('points', 'visibility', showLine ? 'none' : 'visible');
    _map.setLayoutProperty('clusters', 'visibility', showLine ? 'none' : 'visible');
    _map.setLayoutProperty('cluster-count', 'visibility', showLine ? 'none' : 'visible');
}

function applyCurrentPosition() {
    if (!_loaded || !_pending.currentPos) return;
    _map.getSource('current-position').setData({
        type: 'FeatureCollection',
        features: [{ type: 'Feature', geometry: { type: 'Point', coordinates: _pending.currentPos }, properties: {} }],
    });
}

// Fits the view to the point set - only called when the underlying data changes
// (not on a render-mode/style toggle), so toggling controls never jumps the map.
function fitToPoints() {
    if (!_loaded || _pending.points.length === 0) return;
    let minLon = Infinity, minLat = Infinity, maxLon = -Infinity, maxLat = -Infinity;
    for (const p of _pending.points) {
        if (p.longitude < minLon) minLon = p.longitude;
        if (p.longitude > maxLon) maxLon = p.longitude;
        if (p.latitude < minLat) minLat = p.latitude;
        if (p.latitude > maxLat) maxLat = p.latitude;
    }
    _map.fitBounds([[minLon, minLat], [maxLon, maxLat]], { padding: 48, maxZoom: 15, duration: 0 });
}

// MapTiler's base styles label everything in the local language by default (a
// Vienna street shows as it would in German, Tokyo in Japanese, etc.). Their own
// SDK offers a `language` option that does this same rewrite client-side - since
// we're driving raw MapLibre instead, we do it by hand: every symbol layer's
// text-field is repointed at the `name:en` field their vector tiles carry
// alongside the local `name`, falling back to `name` when no English variant
// exists (still better than a blank label). Re-run on every style (re)load, since
// each base style ships its own fresh set of label layers.
function useEnglishLabels() {
    const style = _map.getStyle();
    if (!style || !style.layers) return;
    for (const layer of style.layers) {
        if (layer.type !== 'symbol') continue;
        const textField = layer.layout && layer.layout['text-field'];
        if (!textField || !JSON.stringify(textField).includes('name')) continue;
        _map.setLayoutProperty(layer.id, 'text-field', ['coalesce', ['get', 'name:en'], ['get', 'name']]);
    }
}

// `cluster` can't be toggled on an existing source - it's fixed at source-creation
// time - so both the clustered points source and the line source are created once
// and kept in sync via setData(); the render-mode toggle only flips layer
// visibility. Re-run after every style change too, since setStyle() drops custom
// sources/layers.
function initLayers() {
    _loaded = true;
    useEnglishLabels();

    if (!_map.getSource('location-points')) {
        _map.addSource('location-points', {
            type: 'geojson', data: emptyFC(), cluster: true, clusterRadius: 40, clusterMaxZoom: 14,
        });
    }
    if (!_map.getSource('location-line')) {
        _map.addSource('location-line', { type: 'geojson', data: emptyFC() });
    }
    if (!_map.getSource('current-position')) {
        _map.addSource('current-position', { type: 'geojson', data: emptyFC() });
    }

    if (!_map.getLayer('route-line')) {
        _map.addLayer({
            id: 'route-line', type: 'line', source: 'location-line',
            layout: { 'line-join': 'round', 'line-cap': 'round' },
            paint: { 'line-color': '#00b4d8', 'line-width': 3 },
        });
    }
    if (!_map.getLayer('clusters')) {
        _map.addLayer({
            id: 'clusters', type: 'circle', source: 'location-points', filter: ['has', 'point_count'],
            paint: {
                'circle-color': '#00b4d8',
                'circle-radius': ['step', ['get', 'point_count'], 14, 10, 20, 50, 28],
            },
        });
    }
    if (!_map.getLayer('cluster-count')) {
        _map.addLayer({
            id: 'cluster-count', type: 'symbol', source: 'location-points', filter: ['has', 'point_count'],
            layout: { 'text-field': ['get', 'point_count_abbreviated'], 'text-size': 12 },
            paint: { 'text-color': '#fff' },
        });
    }
    if (!_map.getLayer('points')) {
        _map.addLayer({
            id: 'points', type: 'circle', source: 'location-points', filter: ['!', ['has', 'point_count']],
            paint: {
                'circle-radius': 5,
                'circle-color': ['case', ['get', 'flagged'], '#e94560', '#90be6d'],
                'circle-stroke-width': 1,
                'circle-stroke-color': '#16213e',
            },
        });
    }
    if (!_map.getLayer('current-position-layer')) {
        _map.addLayer({
            id: 'current-position-layer', type: 'circle', source: 'current-position',
            paint: { 'circle-radius': 8, 'circle-color': '#4361ee', 'circle-stroke-width': 3, 'circle-stroke-color': '#fff' },
        });
    }

    applyData();
    applyVisibility();
    applyCurrentPosition();
}

export function initMap(containerId, apiKey, styleKey) {
    _apiKey = apiKey;
    _map = new maplibregl.Map({
        container: containerId,
        style: styleFor(styleKey),
        center: [10, 20],
        zoom: 1,
    });
    _map.addControl(new maplibregl.NavigationControl());
    _map.on('load', initLayers);
}

export function setMapStyle(styleKey) {
    if (!_map) return;
    _loaded = false;
    _map.once('styledata', initLayers);
    _map.setStyle(styleFor(styleKey));
}

// `pointsJson` is a JSON array of the DTOs returned by GET /api/locations
// (latitude/longitude/timestamp_epoch_ms/is_flagged). Thinned deterministically if
// the range grows past MAX_RENDERED_POINTS - a render-time safeguard only, it never
// touches the outlier list or the underlying dataset.
export function renderPoints(pointsJson, mode) {
    let points = JSON.parse(pointsJson);
    if (points.length > MAX_RENDERED_POINTS) {
        const step = Math.ceil(points.length / MAX_RENDERED_POINTS);
        points = points.filter((_, i) => i % step === 0);
    }
    _pending.points = points;
    _pending.mode = mode;
    applyData();
    applyVisibility();
    fitToPoints();
}

export function setRenderMode(mode) {
    _pending.mode = mode;
    applyVisibility();
}

export function renderCurrentPosition(lat, lon) {
    _pending.currentPos = [lon, lat];
    applyCurrentPosition();
}

export function flyToCurrentPosition() {
    if (_map && _pending.currentPos) {
        _map.flyTo({ center: _pending.currentPos, zoom: 14 });
    }
}

export function startWatchingPosition(onUpdate, onError) {
    if (!navigator.geolocation) {
        onError("Geolocation not supported by this browser");
        return null;
    }
    return navigator.geolocation.watchPosition(
        pos => onUpdate(pos.coords.latitude, pos.coords.longitude),
        err => onError(err.message),
        { enableHighAccuracy: false, maximumAge: 15000, timeout: 20000 }
    );
}

export function stopWatchingPosition(id) {
    if (id !== null && id !== undefined) navigator.geolocation.clearWatch(id);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = initMap)]
    pub fn init_map(container_id: &str, api_key: &str, style_key: &str);

    #[wasm_bindgen(js_name = setMapStyle)]
    pub fn set_map_style(style_key: &str);

    #[wasm_bindgen(js_name = renderPoints)]
    pub fn render_points(points_json: &str, mode: &str);

    #[wasm_bindgen(js_name = setRenderMode)]
    pub fn set_render_mode(mode: &str);

    #[wasm_bindgen(js_name = renderCurrentPosition)]
    pub fn render_current_position(lat: f64, lon: f64);

    #[wasm_bindgen(js_name = flyToCurrentPosition)]
    pub fn fly_to_current_position();

    #[wasm_bindgen(js_name = startWatchingPosition)]
    pub fn start_watching_position(
        on_update: &Closure<dyn FnMut(f64, f64)>,
        on_error: &Closure<dyn FnMut(String)>,
    ) -> JsValue;

    #[wasm_bindgen(js_name = stopWatchingPosition)]
    pub fn stop_watching_position(watch_id: JsValue);
}
