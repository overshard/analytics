import Chart from "chart.js/auto";

// Palette matches the warm-earth SCSS: mossy green primary, amber secondary,
// terracotta accent, plus neutral earth tones for the longer tails of the
// doughnut charts. Alpha-blended so adjacent segments stay readable.
const palette = [
  "rgba(107, 158, 120, 0.75)", // green
  "rgba(201, 168, 76, 0.75)",  // amber
  "rgba(196, 112, 85, 0.75)",  // terracotta
  "rgba(126, 170, 184, 0.75)", // info blue-slate
  "rgba(160, 152, 144, 0.75)", // warm grey
  "rgba(125, 184, 140, 0.75)", // green bright
  "rgba(221, 192, 106, 0.7)",  // amber bright
  "rgba(216, 136, 112, 0.7)",  // terracotta bright
  "rgba(132, 124, 114, 0.7)",  // gray-400
  "rgba(196, 189, 178, 0.7)",  // gray-200
];

const paletteBorders = palette.map((c) => c.replace(/0?\.\d+\)/, "1)"));

const fontStack = "'Monaspace Argon', ui-monospace, 'Cascadia Code', Consolas, monospace";

// Common axis / grid styling for the dark theme.
Chart.defaults.color = "rgba(221, 215, 205, 0.55)";
Chart.defaults.borderColor = "rgba(107, 158, 120, 0.08)";
Chart.defaults.font.family = fontStack;
Chart.defaults.font.size = 11;

const tooltipStyle = {
  backgroundColor: "rgba(9, 8, 6, 0.95)",
  borderColor: "rgba(107, 158, 120, 0.3)",
  borderWidth: 1,
  titleColor: "#ede8e0",
  bodyColor: "#ddd7cd",
  padding: 10,
  titleFont: { family: fontStack, size: 12 },
  bodyFont: { family: fontStack, size: 11 },
  cornerRadius: 4,
};

// Normalize label widths across the four doughnut charts so their legends line
// up in the sidebar. Pad with non-breaking spaces so monospace widths match.
let maxLabelLength = 0;
if (document.getElementById("chart-total-events-by-browser-data")) {
  const browserData = JSON.parse(document.getElementById("chart-total-events-by-browser-data").innerHTML);
  const deviceData = JSON.parse(document.getElementById("chart-total-events-by-device-data").innerHTML);
  const screenSizeData = JSON.parse(document.getElementById("chart-total-events-by-screen-size-data").innerHTML);
  const platformData = JSON.parse(document.getElementById("chart-total-events-by-platform-data").innerHTML);
  const allData = [...browserData, ...deviceData, ...screenSizeData, ...platformData];
  for (let i = 0; i < allData.length; i++) {
    if (allData[i].label.length > maxLabelLength) {
      maxLabelLength = allData[i].label.length;
    }
  }
}

function padLabels(data) {
  for (let i = 0; i < data.length; i++) {
    data[i].label = data[i].label + " ".repeat(Math.max(0, maxLabelLength - data[i].label.length));
  }
  return data;
}

const doughnutOptions = {
  responsive: true,
  aspectRatio: 1.8,
  animation: { animateRotate: false },
  cutout: "60%",
  plugins: {
    tooltip: tooltipStyle,
    legend: {
      position: "right",
      labels: {
        boxWidth: 8,
        boxHeight: 8,
        padding: 8,
        color: "rgba(221, 215, 205, 0.7)",
        font: { family: fontStack, size: 11 },
      },
    },
  },
};

function renderDoughnut(canvasId, dataId) {
  document.addEventListener("DOMContentLoaded", function () {
    const canvas = document.getElementById(canvasId);
    if (!canvas) return;
    const raw = JSON.parse(document.getElementById(dataId).innerHTML);
    const data = padLabels(raw);
    new Chart(canvas.getContext("2d"), {
      type: "doughnut",
      data: {
        labels: data.map((d) => d.label),
        datasets: [
          {
            data: data.map((d) => d.count),
            backgroundColor: palette,
            borderColor: "rgba(14, 13, 10, 0.9)",
            borderWidth: 2,
          },
        ],
      },
      options: doughnutOptions,
    });
  });
}

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events");
  if (!canvas) return;
  const data = JSON.parse(document.getElementById("chart-total-events-data").innerHTML);
  const ctx = canvas.getContext("2d");

  // Build a subtle vertical gradient fill so the line chart feels lit from
  // below without washing out the dark surface.
  const gradient = ctx.createLinearGradient(0, 0, 0, 320);
  gradient.addColorStop(0, "rgba(107, 158, 120, 0.35)");
  gradient.addColorStop(1, "rgba(107, 158, 120, 0.01)");

  new Chart(ctx, {
    type: "line",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          label: "events",
          data: data.map((d) => d.count),
          backgroundColor: gradient,
          borderColor: "rgba(125, 184, 140, 0.95)",
          pointBackgroundColor: "rgba(125, 184, 140, 1)",
          pointBorderColor: "rgba(14, 13, 10, 1)",
          pointRadius: 3,
          pointHoverRadius: 5,
          borderWidth: 2,
          tension: 0.25,
          fill: true,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: { duration: 0 },
      plugins: {
        tooltip: { ...tooltipStyle, mode: "index", intersect: false },
        legend: {
          display: false,
        },
      },
      scales: {
        x: {
          ticks: {
            autoSkip: true,
            maxTicksLimit: 10,
            maxRotation: 0,
            color: "rgba(132, 124, 114, 0.85)",
            font: { family: fontStack, size: 10 },
          },
          grid: { color: "rgba(107, 158, 120, 0.04)", drawTicks: false },
          border: { color: "rgba(107, 158, 120, 0.12)" },
        },
        y: {
          beginAtZero: true,
          ticks: {
            color: "rgba(132, 124, 114, 0.85)",
            font: { family: fontStack, size: 10 },
          },
          grid: { color: "rgba(107, 158, 120, 0.06)", drawTicks: false },
          border: { display: false },
        },
      },
    },
  });
});

renderDoughnut("chart-total-events-by-browser", "chart-total-events-by-browser-data");
renderDoughnut("chart-total-events-by-device", "chart-total-events-by-device-data");
renderDoughnut("chart-total-events-by-screen-size", "chart-total-events-by-screen-size-data");
renderDoughnut("chart-total-events-by-platform", "chart-total-events-by-platform-data");
