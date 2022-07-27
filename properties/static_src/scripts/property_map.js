import { scaleLinear } from "d3-scale";
import Datamap from "datamaps/dist/datamaps.all.min";

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
        return (`
          <div class="tooltip show" role="tooltip">
            <div class="tooltip-inner text-nowrap">
              <strong class="d-block">${geo.properties.name}</strong>
              ${data.numberOfThings} session start${data.numberOfThings === 1 ? "" : "s"}
            </div>
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
