import { scaleLinear } from "d3-scale";
import Datamap from "datamaps/dist/datamaps.usa.min";

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
  const colorScale = scaleLinear().domain([0, max]).range(["#cfe2ff", "#031633"]);

  for (const region in data) {
    data[region].fillColor = colorScale(data[region].numberOfThings);
  }

  const datamap = new Datamap({
    element: datamapEl,
    scope: "usa",
    responsive: true,
    fills: {
      defaultFill: "rgba(108, 117, 125, 0.4)",
    },
    geographyConfig: {
      highlightFillColor: "#0d6efd",
      popupTemplate: function (geo, data) {
        const count = data && data.numberOfThings != null ? data.numberOfThings : 0;
        return (`
          <div style="position:relative;padding:4px 10px;background:rgba(0,0,0,0.85);color:#fff;-webkit-text-fill-color:#fff;border-radius:4px;font-size:0.875rem;white-space:nowrap;pointer-events:none;">
            <span style="display:block;font-weight:bold;">${geo.properties.name}</span>
            ${count} session start${count === 1 ? "" : "s"}
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
