// Build static map assets from Natural Earth.
//
// Downloads admin-0 (countries) and admin-1 (states / provinces) GeoJSON,
// converts to TopoJSON with quantization, and writes:
//
//   analytics/static_maps/world.json            - all countries, keyed by ISO_A2
//   analytics/static_maps/admin1/{ISO_A2}.json  - one file per country
//
// Source: martynafford/natural-earth-geojson (mirrors Natural Earth public-
// domain data as GeoJSON). Run at Docker build time so the produced files
// are baked into the image — no runtime third-party calls.
//
// Run with `bun run build:maps`.

import { mkdir, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { topology } from "topojson-server";

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUT_DIR = resolve(__dirname, "../static_maps");

// 110m for the always-on world view (small, ~30 KB topojson). 10m for
// admin-1 because the 50m and 110m bundles only ship four big countries —
// 10m is the only Natural Earth tier with full per-country admin-1 coverage.
const BASE = "https://raw.githubusercontent.com/martynafford/natural-earth-geojson/master";
const ADMIN0_URL = `${BASE}/110m/cultural/ne_110m_admin_0_countries.json`;
const ADMIN1_URL = `${BASE}/10m/cultural/ne_10m_admin_1_states_provinces.json`;

// 1e5 keeps coastlines smooth at typical screen sizes while still cutting
// file size by ~70% versus raw GeoJSON. d3-geo handles the dequantization.
const QUANTIZATION = 1e5;

async function fetchJson(url) {
  process.stdout.write(`  GET ${url}\n`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} - ${url}`);
  return res.json();
}

// Natural Earth has a long-running bug where France ("FRA") and Norway
// ("NOR") get ISO_A2 = "-99" because of an EU/Schengen dispute baked into
// the source. Map them back from ISO_A3 — every other country with a real
// ISO_A2 ships it cleanly.
const A3_TO_A2_OVERRIDES = {
  FRA: "FR",
  NOR: "NO",
};

function normalizeCountryCode(props) {
  const code = props.ISO_A2 ?? props.iso_a2;
  if (code && code !== "-99" && code !== -99) return code;
  const eh = props.ISO_A2_EH ?? props.iso_a2_eh;
  if (eh && eh !== "-99" && eh !== -99) return eh;
  // ISO_A3 is also "-99" for the same disputed records, so fall back to
  // ADM0_A3 which Natural Earth always populates.
  const a3 = props.ADM0_A3 ?? props.adm0_a3 ?? props.ISO_A3 ?? props.iso_a3;
  if (a3 && a3 !== "-99" && A3_TO_A2_OVERRIDES[a3]) return A3_TO_A2_OVERRIDES[a3];
  return null;
}

function trimCountryProps(feature) {
  // World-map features only need a name and ISO code on the client; drop
  // the other ~80 Natural Earth fields to keep the payload small.
  const p = feature.properties || {};
  const iso = normalizeCountryCode(p);
  return {
    ...feature,
    id: iso,
    properties: {
      iso: iso,
      name: p.NAME ?? p.ADMIN ?? p.name ?? "",
    },
  };
}

function trimAdmin1Props(feature) {
  const p = feature.properties || {};
  // Natural Earth's `name` is the local-language form (e.g. "Bayern"),
  // while DB-IP / MaxMind return the English form ("Bavaria"). Keep both
  // so the runtime lookup can match either.
  return {
    ...feature,
    properties: {
      iso_3166_2: p.iso_3166_2 ?? "",
      postal: p.postal ?? "",
      name: p.name ?? "",
      name_alt: p.name_alt ?? "",
    },
  };
}

async function buildWorld(admin0) {
  const features = admin0.features
    .map(trimCountryProps)
    .filter((f) => f.id);
  const topo = topology({ countries: { type: "FeatureCollection", features } }, QUANTIZATION);
  const path = resolve(OUT_DIR, "world.json");
  await writeFile(path, JSON.stringify(topo));
  return { path, count: features.length, bytes: JSON.stringify(topo).length };
}

async function buildAdmin1(admin1) {
  const byCountry = new Map();
  for (const f of admin1.features) {
    const iso = normalizeCountryCode(f.properties || {});
    if (!iso) continue;
    if (!byCountry.has(iso)) byCountry.set(iso, []);
    byCountry.get(iso).push(trimAdmin1Props(f));
  }

  await mkdir(resolve(OUT_DIR, "admin1"), { recursive: true });

  let totalBytes = 0;
  for (const [iso, features] of byCountry) {
    const topo = topology({ regions: { type: "FeatureCollection", features } }, QUANTIZATION);
    const json = JSON.stringify(topo);
    await writeFile(resolve(OUT_DIR, "admin1", `${iso}.json`), json);
    totalBytes += json.length;
  }
  return { count: byCountry.size, bytes: totalBytes };
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });

  console.log("Downloading Natural Earth source data...");
  const [admin0, admin1] = await Promise.all([
    fetchJson(ADMIN0_URL),
    fetchJson(ADMIN1_URL),
  ]);

  console.log("Building world topology...");
  const world = await buildWorld(admin0);
  console.log(`  ${world.count} countries -> ${world.path} (${(world.bytes / 1024).toFixed(1)} KB)`);

  console.log("Building per-country admin-1 topologies...");
  const a1 = await buildAdmin1(admin1);
  console.log(`  ${a1.count} country files (${(a1.bytes / 1024).toFixed(1)} KB total)`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
