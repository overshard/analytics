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
        if (!data || data.numberOfThings == null) return "";
        const count = data.numberOfThings;
        return (`
          <div style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif;font-style:normal;font-weight:normal;line-height:1.4;padding:4px 10px;background:rgba(0,0,0,0.9);border-radius:4px;font-size:14px;white-space:nowrap;pointer-events:none;">
            <span style="display:block;font-weight:bold;color:#fff;-webkit-text-fill-color:#fff;">${geo.properties.name}</span>
            <span style="color:#fff;-webkit-text-fill-color:#fff;">${count} session start${count === 1 ? "" : "s"}</span>
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
