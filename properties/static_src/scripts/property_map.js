import { scaleLinear } from "d3-scale";
import Datamap from "datamaps/dist/datamaps.usa";

document.addEventListener("DOMContentLoaded", function () {
  const datamapEl = document.getElementById("datamap");
  if (!datamapEl) return;

  const data = JSON.parse(
    document.getElementById("chart-total-session-starts-by-region-data").innerHTML
  );

  let max = 0;
  for (const region in data) {
    if (data[region].numberOfThings > max) {
      max = data[region].numberOfThings;
    }
  }
  // Warm-earth choropleth: faint green for low volume, bright green for the
  // busiest states. Matches the rest of the dashboard palette.
  const colorScale = scaleLinear()
    .domain([0, max])
    .range(["rgba(107, 158, 120, 0.12)", "rgba(125, 184, 140, 0.95)"]);

  for (const region in data) {
    data[region].fillColor = colorScale(data[region].numberOfThings);
  }

  const datamap = new Datamap({
    element: datamapEl,
    scope: "usa",
    responsive: true,
    fills: {
      defaultFill: "rgba(107, 158, 120, 0.06)",
    },
    geographyConfig: {
      borderColor: "rgba(107, 158, 120, 0.18)",
      borderWidth: 0.6,
      highlightFillColor: "#c9a84c",
      highlightBorderColor: "rgba(201, 168, 76, 0.6)",
      popupTemplate: function (geo, data) {
        if (!data || data.numberOfThings == null) return "";
        const count = data.numberOfThings;
        return (`
          <div style="font-family:'Monaspace Argon',ui-monospace,monospace;line-height:1.4;padding:6px 10px;background:#13120e;border:1px solid rgba(107, 158, 120, 0.3);border-radius:4px;font-size:12px;white-space:nowrap;pointer-events:none;color:#ddd7cd;">
            <span style="display:block;font-weight:600;color:#ede8e0;-webkit-text-fill-color:#ede8e0;letter-spacing:0.02em;">${geo.properties.name}</span>
            <span style="color:#c9a84c;-webkit-text-fill-color:#c9a84c;">${count} session${count === 1 ? "" : "s"}</span>
          </div>
        `);
      },
    },
    data: data,
  });

  datamapEl.querySelector("svg").style.display = "block";

  window.addEventListener("resize", function () {
    datamap.resize();
  });
});
