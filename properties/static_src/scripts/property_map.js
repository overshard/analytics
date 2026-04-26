// World choropleth with click-to-drill-down to admin-1 (states / provinces /
// regions). Replaces the abandoned `datamaps` library with vanilla d3-geo +
// topojson-client. Country shapes are baked into the image; per-country
// admin-1 topojson is lazy-fetched on click.

import { geoNaturalEarth1, geoMercator, geoAlbersUsa, geoPath } from "d3-geo";
import { scaleLinear } from "d3-scale";
import { select } from "d3-selection";
import { feature } from "topojson-client";

const STATIC_BASE = "/static";
const WORLD_URL = `${STATIC_BASE}/world.json`;
const ADMIN1_URL = (iso) => `${STATIC_BASE}/admin1/${iso}.json`;

const FILL_LOW = "rgba(107, 158, 120, 0.12)";
const FILL_HIGH = "rgba(125, 184, 140, 0.95)";
const FILL_DEFAULT = "rgba(107, 158, 120, 0.06)";
const STROKE = "rgba(107, 158, 120, 0.18)";
const STROKE_HIGHLIGHT = "rgba(201, 168, 76, 0.6)";
const FILL_HIGHLIGHT = "#c9a84c";

document.addEventListener("DOMContentLoaded", () => {
  const root = document.getElementById("map");
  if (!root) return;

  const byCountry = readJsonScript("map-session-starts-by-country") || {};
  const byCountryRegion = readJsonScript("map-session-starts-by-country-region") || {};
  const titleEl = document.getElementById("map-title");
  const backEl = document.getElementById("map-back");

  const tooltip = createTooltip(root);

  // Cache fetched admin-1 topojson per country so re-clicking is instant.
  const admin1Cache = new Map();
  let worldData = null;

  const state = { view: "world", country: null };

  fetch(WORLD_URL)
    .then((r) => r.json())
    .then((topo) => {
      worldData = topo;
      renderWorld();
    })
    .catch((err) => {
      showFallback(`map unavailable: ${err.message}`);
    });

  function showFallback(message) {
    root.querySelectorAll("svg").forEach((s) => s.remove());
    let fallback = root.querySelector(".map-fallback");
    if (!fallback) {
      fallback = document.createElement("div");
      fallback.className = "map-fallback";
      Object.assign(fallback.style, { padding: "1rem", color: "#ddd7cd", fontSize: "12px" });
      root.appendChild(fallback);
    }
    fallback.textContent = message;
  }

  backEl.addEventListener("click", () => {
    state.view = "world";
    state.country = null;
    titleEl.textContent = "sessions · world";
    backEl.hidden = true;
    renderWorld();
  });

  function renderWorld() {
    const countries = feature(worldData, worldData.objects.countries);
    const max = Math.max(0, ...Object.values(byCountry));
    const color = scaleLinear().domain([0, max || 1]).range([FILL_LOW, FILL_HIGH]);

    drawMap({
      features: countries.features,
      projection: geoNaturalEarth1(),
      fillFor: (f) => {
        const count = byCountry[f.properties.iso] || 0;
        return count ? color(count) : FILL_DEFAULT;
      },
      labelFor: (f) => f.properties.name,
      countFor: (f) => byCountry[f.properties.iso] || 0,
      onClick: (f) => {
        if (f.properties.iso) drillDown(f.properties.iso, f.properties.name);
      },
      clickable: (f) => Boolean(byCountryRegion[f.properties.iso]),
    });
  }

  async function drillDown(iso, name) {
    titleEl.textContent = `sessions · ${name.toLowerCase()}`;
    backEl.hidden = false;
    state.view = "country";
    state.country = iso;

    let topo = admin1Cache.get(iso);
    if (!topo) {
      try {
        const res = await fetch(ADMIN1_URL(iso));
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        topo = await res.json();
        admin1Cache.set(iso, topo);
      } catch (err) {
        showFallback(`no detail map for ${name}`);
        return;
      }
    }

    const regions = feature(topo, topo.objects.regions);
    const counts = byCountryRegion[iso] || {};
    const max = Math.max(0, ...Object.values(counts));
    const color = scaleLinear().domain([0, max || 1]).range([FILL_LOW, FILL_HIGH]);

    drawMap({
      features: regions.features,
      // geoAlbersUsa insets Alaska + Hawaii so they don't overwhelm the
      // viewport. geoMercator works fine for most other countries (a bit of
      // distortion at high latitudes, but readable).
      projection: iso === "US" ? geoAlbersUsa() : geoMercator(),
      fillFor: (f) => {
        const count = lookupRegionCount(counts, f.properties);
        return count ? color(count) : FILL_DEFAULT;
      },
      labelFor: (f) => f.properties.name,
      countFor: (f) => lookupRegionCount(counts, f.properties),
      onClick: () => {},
      clickable: () => false,
    });
  }

  function drawMap({ features: feats, projection, fillFor, labelFor, countFor, onClick, clickable }) {
    // Only clear the previous SVG and any fallback message — leave the
    // tooltip element in place so its event handlers and stable position
    // survive across redraws.
    root.querySelectorAll("svg, .map-fallback").forEach((el) => el.remove());
    const { width, height } = root.getBoundingClientRect();
    const w = Math.max(width, 320);
    const h = Math.max(height, 320);

    projection.fitSize([w, h], { type: "FeatureCollection", features: feats });
    const path = geoPath(projection);

    const svg = select(root)
      .append("svg")
      .attr("viewBox", `0 0 ${w} ${h}`)
      .attr("preserveAspectRatio", "xMidYMid meet")
      .style("width", "100%")
      .style("height", "100%")
      .style("display", "block");

    svg
      .append("g")
      .selectAll("path")
      .data(feats)
      .join("path")
      .attr("d", path)
      .attr("fill", fillFor)
      .attr("stroke", STROKE)
      .attr("stroke-width", 0.6)
      .style("cursor", (f) => (clickable(f) ? "pointer" : "default"))
      .on("mouseenter", function (event, f) {
        const count = countFor(f);
        select(this).attr("fill", FILL_HIGHLIGHT).attr("stroke", STROKE_HIGHLIGHT);
        tooltip.show(labelFor(f), count, event);
      })
      .on("mousemove", (event) => tooltip.move(event))
      .on("mouseleave", function (event, f) {
        select(this).attr("fill", fillFor(f)).attr("stroke", STROKE);
        tooltip.hide();
      })
      .on("click", (_event, f) => onClick(f));
  }
});

function readJsonScript(id) {
  const el = document.getElementById(id);
  if (!el) return null;
  try {
    return JSON.parse(el.textContent);
  } catch {
    return null;
  }
}

function lookupRegionCount(counts, props) {
  // GeoIP backends report region differently across providers and even
  // across rows: it can be the ISO subdivision code ("CA"), the full
  // English name ("California" / "Bavaria"), the local-language name
  // ("Bayern"), or the iso_3166_2 form ("US-CA"). Try each Natural Earth
  // alias until one matches.
  const tries = [
    props.postal,
    props.iso_3166_2,
    props.name,
    props.name_alt,
  ];
  for (const key of tries) {
    if (key && counts[key] != null) return counts[key];
  }
  // Some readers store just the suffix of iso_3166_2 (e.g. "CA" not "US-CA").
  if (props.iso_3166_2) {
    const tail = props.iso_3166_2.split("-")[1];
    if (counts[tail] != null) return counts[tail];
  }
  return 0;
}

function createTooltip(root) {
  const el = document.createElement("div");
  Object.assign(el.style, {
    position: "absolute",
    pointerEvents: "none",
    background: "#13120e",
    border: "1px solid rgba(107, 158, 120, 0.3)",
    borderRadius: "4px",
    padding: "6px 10px",
    fontFamily: "'Monaspace Argon', ui-monospace, monospace",
    fontSize: "12px",
    color: "#ddd7cd",
    whiteSpace: "nowrap",
    lineHeight: "1.4",
    transform: "translate(-50%, calc(-100% - 8px))",
    opacity: "0",
    transition: "opacity 80ms",
    zIndex: "10",
  });
  root.appendChild(el);

  return {
    show(label, count, event) {
      el.innerHTML =
        `<span style="display:block;font-weight:600;color:#ede8e0;letter-spacing:0.02em;">${escape(label)}</span>` +
        `<span style="color:#c9a84c;">${count} session${count === 1 ? "" : "s"}</span>`;
      this.move(event);
      el.style.opacity = "1";
    },
    move(event) {
      const rect = root.getBoundingClientRect();
      el.style.left = `${event.clientX - rect.left}px`;
      el.style.top = `${event.clientY - rect.top}px`;
    },
    hide() {
      el.style.opacity = "0";
    },
  };
}

function escape(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
