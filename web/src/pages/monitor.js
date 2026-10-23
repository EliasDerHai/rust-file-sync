const CHARTS = [
  {
    id: "monitor-usage",
    axes: { y: { min: 0, suggestedMax: 100, fmt: (v) => v.toFixed(1) + " %" } },
    series: [
      { key: "sys_cpu", label: "System CPU", color: "#00b4d8" },
      { key: "app_cpu", label: "App CPU", color: "#90be6d" },
      { key: "sys_mem", label: "System memory", color: "#e94560" },
      { key: "app_mem", label: "App memory", color: "#b388ff" },
      { key: "swap", label: "Swap", color: "#ffd166" },
      { key: "disk_used", label: "Disk used", color: "#f4a261", plain: true },
    ],
  },
  {
    id: "monitor-thermal",
    axes: {
      y: { fmt: (v) => v.toFixed(1) + " °C" },
      y1: { min: 0, position: "right", fmt: (v) => v.toFixed(2) },
    },
    series: [
      { key: "cpu_temp", label: "CPU temperature", color: "#ff6b6b" },
      { key: "load1", label: "Load (1 min)", color: "#4cc9f0", axis: "y1" },
    ],
  },
  {
    id: "monitor-network",
    axes: { y: { min: 0, fmt: fmtRate } },
    series: [
      { key: "net_rx", label: "Received", color: "#06d6a0" },
      { key: "net_tx", label: "Sent", color: "#ef476f" },
    ],
  },
  {
    id: "monitor-disk",
    axes: {
      y: { fmt: (v) => v.toFixed(1) + " GiB" },
      y1: { min: 0, position: "right", fmt: fmtRate },
    },
    series: [
      {
        key: "disk_free_gib",
        label: "Disk free",
        color: "#f4a261",
        plain: true,
      },
      { key: "app_read", label: "App read", color: "#118ab2", axis: "y1" },
      { key: "app_write", label: "App write", color: "#e76f51", axis: "y1" },
    ],
  },
];

const _charts = {};
let _debounce = null;

function fmtRate(v) {
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let i = 0;
  while (Math.abs(v) >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return v.toFixed(i === 0 ? 0 : 1) + " " + units[i];
}

// a bucket with gaps on both sides draws no line segment - give it a dot instead
function isolatedDot(ctx) {
  const d = ctx.dataset.data,
    i = ctx.dataIndex;
  const missing = (p) => !p || p.y === null;
  return !missing(d[i]) && missing(d[i - 1]) && missing(d[i + 1]) ? 2 : 0;
}

function hexAlpha(hex, alpha) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
}

// datasets per series: [max, min, avg] for stat metrics (band = max filled to min),
// [value] for plain ones. `_group` ties a band to its line for legend toggling.
function datasetsFor(cfg) {
  const out = [];
  for (const s of cfg.series) {
    const axis = s.axis || "y";
    const common = {
      yAxisID: axis,
      pointRadius: 0,
      borderWidth: 1.5,
      tension: 0.2,
      spanGaps: false,
      _group: s.key,
    };
    if (!s.plain) {
      out.push({
        ...common,
        label: s.label + " max",
        data: [],
        borderWidth: 0,
        fill: "+1",
        backgroundColor: hexAlpha(s.color, 0.15),
        _band: "max",
      });
      out.push({
        ...common,
        label: s.label + " min",
        data: [],
        borderWidth: 0,
        fill: false,
        _band: "min",
      });
    }
    out.push({
      ...common,
      label: s.label,
      data: [],
      borderColor: s.color,
      backgroundColor: s.color,
      fill: false,
      pointRadius: isolatedDot,
      _fmt: cfg.axes[axis].fmt,
    });
  }
  return out;
}

function syncWindow(source, min, max) {
  for (const chart of Object.values(_charts)) {
    if (chart === source) continue;
    chart.options.scales.x.min = min;
    chart.options.scales.x.max = max;
    chart.update("none");
  }
}

export function initMonitorCharts(onRange) {
  destroyMonitorCharts();
  const onWindowChange = ({ chart }) => {
    const { min, max } = chart.scales.x;
    syncWindow(chart, min, max);
    clearTimeout(_debounce);
    _debounce = setTimeout(() => onRange(min, max), 300);
  };
  // phones: less chrome around the plot (the secondary axis is dropped, tooltips still show its units)
  const narrow = window.matchMedia("(max-width: 640px)").matches;
  const tickFont = { size: narrow ? 10 : 12 };
  for (const cfg of CHARTS) {
    const canvas = document.getElementById(cfg.id);
    if (!canvas) continue;
    const scales = {
      x: {
        type: "time",
        time: {
          displayFormats: {
            hour: "HH:mm",
            minute: "HH:mm",
            second: "HH:mm:ss",
          },
        },
        ticks: {
          color: "#aaa",
          maxRotation: 0,
          font: tickFont,
          maxTicksLimit: narrow ? 4 : undefined,
        },
        grid: { color: "#333" },
      },
    };
    for (const [id, a] of Object.entries(cfg.axes)) {
      scales[id] = {
        min: a.min,
        suggestedMax: a.suggestedMax,
        position: a.position || "left",
        display: !(narrow && id !== "y"),
        ticks: {
          color: "#aaa",
          callback: a.fmt,
          font: tickFont,
          maxTicksLimit: narrow ? 5 : undefined,
        },
        grid: { color: id === "y" ? "#333" : "transparent" },
      };
    }
    _charts[cfg.id] = new Chart(canvas, {
      type: "line",
      data: { datasets: datasetsFor(cfg) },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        parsing: false,
        normalized: true,
        interaction: { mode: "index", intersect: false },
        scales,
        plugins: {
          legend: {
            position: narrow ? "bottom" : "top",
            labels: {
              color: "#eee",
              boxWidth: narrow ? 12 : 40,
              font: { size: narrow ? 11 : 12 },
              filter: (item, data) => !data.datasets[item.datasetIndex]._band,
            },
            onClick: (e, item, legend) => {
              const chart = legend.chart;
              const group = chart.data.datasets[item.datasetIndex]._group;
              const visible = chart.isDatasetVisible(item.datasetIndex);
              chart.data.datasets.forEach((d, i) => {
                if (d._group === group) chart.setDatasetVisibility(i, !visible);
              });
              chart.update("none");
            },
          },
          tooltip: {
            filter: (item) => !item.dataset._band,
            callbacks: {
              title: (items) =>
                items.length
                  ? new Date(items[0].parsed.x).toLocaleString()
                  : "",
              label: (ctx) => {
                const fmt = ctx.dataset._fmt || ((v) => v);
                const r = ctx.raw;
                if (r.y === null) return null;
                const range =
                  r.min !== undefined && r.min !== r.max
                    ? ` (${fmt(r.min)} – ${fmt(r.max)})`
                    : "";
                return `${ctx.dataset.label}: ${fmt(r.y)}${range}`;
              },
            },
          },
          zoom: {
            zoom: {
              wheel: { enabled: true },
              pinch: { enabled: true },
              mode: "x",
              onZoomComplete: onWindowChange,
            },
            pan: { enabled: true, mode: "x", onPanComplete: onWindowChange },
            limits: { x: { minRange: 60 * 1000 } },
          },
        },
      },
    });
  }
}

// `seriesJson`: MonitorSeriesDto. Shows [viewMin, viewMax] (the data may cover more,
// so panning has something to show before the refetch lands) and limits zoom/pan to
// [limitMin, limitMax].
export function updateMonitorCharts(
  seriesJson,
  viewMin,
  viewMax,
  limitMin,
  limitMax,
) {
  const series = JSON.parse(seriesJson);
  const gap = series.resolution_ms * 2;
  for (const cfg of CHARTS) {
    const chart = _charts[cfg.id];
    if (!chart) continue;
    let i = 0;
    for (const s of cfg.series) {
      const max = [],
        min = [],
        avg = [];
      let prevT = null;
      for (const p of series.points) {
        // a missing bucket means the server was down: break the line
        if (prevT !== null && p.t - prevT > gap) {
          const x = prevT + series.resolution_ms;
          max.push({ x, y: null });
          min.push({ x, y: null });
          avg.push({ x, y: null });
        }
        prevT = p.t;
        const v = p[s.key];
        if (s.plain) {
          avg.push({ x: p.t, y: v ?? null });
        } else {
          max.push({ x: p.t, y: v ? v.max : null });
          min.push({ x: p.t, y: v ? v.min : null });
          avg.push(
            v
              ? { x: p.t, y: v.avg, min: v.min, max: v.max }
              : { x: p.t, y: null },
          );
        }
      }
      if (!s.plain) {
        chart.data.datasets[i++].data = max;
        chart.data.datasets[i++].data = min;
      }
      chart.data.datasets[i++].data = avg;
    }
    chart.options.scales.x.min = viewMin;
    chart.options.scales.x.max = viewMax;
    chart.options.plugins.zoom.limits.x.min = limitMin;
    chart.options.plugins.zoom.limits.x.max = limitMax;
    chart.update("none");
  }
}

export function destroyMonitorCharts() {
  clearTimeout(_debounce);
  for (const [id, chart] of Object.entries(_charts)) {
    chart.destroy();
    delete _charts[id];
  }
}

// plot width in px - one bucket per pixel is as much detail as can be shown
export function monitorChartWidth() {
  const canvas = document.getElementById(CHARTS[0].id);
  return canvas ? canvas.clientWidth : 800;
}
