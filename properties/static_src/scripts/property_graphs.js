import Chart from "chart.js/auto";

const backgroundColors = [
  "rgba(13, 110, 253, 0.4)",
  "rgba(102, 16, 242, 0.4)",
  "rgba(111, 66, 193, 0.4)",
  "rgba(214, 51, 132, 0.4)",
  "rgba(220, 53, 69, 0.4)",
  "rgba(253, 126, 20, 0.4)",
  "rgba(255, 193, 7, 0.4)",
  "rgba(25, 135, 84, 0.4)",
  "rgba(32, 201, 151, 0.4)",
  "rgba(13, 202, 240, 0.4)",
];

const fontStack = 'Consolas, "Andale Mono WT", "Andale Mono", "Lucida Console", "Lucida Sans Typewriter", "DejaVu Sans Mono", "Bitstream Vera Sans Mono", "Liberation Mono", "Nimbus Mono L", Monaco, "Courier New", Courier, monospace';

// Get the max label length from all the datasets
let maxLabelLength = 0;
if (document.getElementById("chart-total-events-by-browser-data")) {
  const browserData = JSON.parse(document.getElementById("chart-total-events-by-browser-data").innerHTML);
  const deviceData = JSON.parse(document.getElementById("chart-total-events-by-device-data").innerHTML);
  const screenSizeData = JSON.parse(document.getElementById("chart-total-events-by-screen-size-data").innerHTML);
  const allData = [...browserData, ...deviceData, ...screenSizeData];
  for (let i = 0; i < allData.length; i++) {
    if (allData[i].label.length > maxLabelLength) {
      maxLabelLength = allData[i].label.length;
    }
  }
}

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events");
  if (!canvas) return;
  const data = JSON.parse(
    document.getElementById("chart-total-events-data").innerHTML
  );
  const ctx = canvas.getContext("2d");
  const chart = new Chart(ctx, {
    type: "line",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          label: "Total events",
          data: data.map((d) => d.count),
          backgroundColor: "rgba(13, 110, 253, 0.4)",
          borderColor: "rgba(13, 110, 253, 0.8)",
          borderWidth: 3,
          fill: true,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: {
        duration: 0,
      },
      plugins: {
        tooltip: {
          mode: "index",
          intersect: false,
        },
        legend: {
          position: "top",
          labels: {
            boxWidth: 10,
            boxHeight: 10,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
      scales: {
        xAxes: {
          ticks: {
            autoSkip: true,
            maxTicksLimit: 10,
            font: {
              size: 12,
              family: fontStack,
            },
            maxRotation: 0,
          },
        },
        yAxes: {
          ticks: {
            beginAtZero: true,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
    },
  });
  chart.canvas.parentNode.style.width = "100%";
  chart.canvas.parentNode.style.height = "300px";
});

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events-by-browser");
  if (!canvas) return;
  const data = JSON.parse(
    document.getElementById("chart-total-events-by-browser-data").innerHTML
  );
  // adjust labels to all be maxLabelLength by adding spaces to the end
  for (let i = 0; i < data.length; i++) {
    data[i].label = data[i].label + " ".repeat(maxLabelLength - data[i].label.length);
  }
  const ctx = canvas.getContext("2d");
  new Chart(ctx, {
    type: "doughnut",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          data: data.map((d) => d.count),
          backgroundColor: backgroundColors,
          borderColor: backgroundColors,
          borderWidth: 1,
        },
      ],
    },
    options: {
      responsive: true,
      aspectRatio: 2,
      animation: {
        animateRotate: false,
      },
      plugins: {
        legend: {
          position: "right",
          labels: {
            boxWidth: 10,
            boxHeight: 10,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
    },
  });
});

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events-by-device");
  if (!canvas) return;
  const data = JSON.parse(
    document.getElementById("chart-total-events-by-device-data").innerHTML
  );
  // adjust labels to all be maxLabelLength by adding spaces to the end
  for (let i = 0; i < data.length; i++) {
    data[i].label = data[i].label + " ".repeat(maxLabelLength - data[i].label.length);
  }
  const ctx = canvas.getContext("2d");
  new Chart(ctx, {
    type: "doughnut",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          data: data.map((d) => d.count),
          backgroundColor: backgroundColors,
          borderColor: backgroundColors,
          borderWidth: 1,
        },
      ],
    },
    options: {
      responsive: true,
      aspectRatio: 2,
      animation: {
        animateRotate: false,
      },
      plugins: {
        legend: {
          position: "right",
          labels: {
            boxWidth: 10,
            boxHeight: 10,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
    },
  });
});

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events-by-screen-size");
  if (!canvas) return;
  const data = JSON.parse(
    document.getElementById("chart-total-events-by-screen-size-data").innerHTML
  );
  // adjust labels to all be maxLabelLength by adding spaces to the end
  for (let i = 0; i < data.length; i++) {
    data[i].label = data[i].label + " ".repeat(maxLabelLength - data[i].label.length);
  }
  const ctx = canvas.getContext("2d");
  new Chart(ctx, {
    type: "doughnut",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          data: data.map((d) => d.count),
          backgroundColor: backgroundColors,
          borderColor: backgroundColors,
          borderWidth: 1,
        },
      ],
    },
    options: {
      responsive: true,
      aspectRatio: 2,
      animation: {
        animateRotate: false,
      },
      plugins: {
        legend: {
          position: "right",
          labels: {
            boxWidth: 10,
            boxHeight: 10,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
    },
  });
});

document.addEventListener("DOMContentLoaded", function () {
  const canvas = document.getElementById("chart-total-events-by-platform");
  if (!canvas) return;
  const data = JSON.parse(
    document.getElementById("chart-total-events-by-platform-data").innerHTML
  );
  // adjust labels to all be maxLabelLength by adding spaces to the end
  for (let i = 0; i < data.length; i++) {
    data[i].label = data[i].label + " ".repeat(maxLabelLength - data[i].label.length);
  }
  const ctx = canvas.getContext("2d");
  new Chart(ctx, {
    type: "doughnut",
    data: {
      labels: data.map((d) => d.label),
      datasets: [
        {
          data: data.map((d) => d.count),
          backgroundColor: backgroundColors,
          borderColor: backgroundColors,
          borderWidth: 1,
        },
      ],
    },
    options: {
      responsive: true,
      aspectRatio: 2,
      animation: {
        animateRotate: false,
      },
      plugins: {
        legend: {
          position: "right",
          labels: {
            boxWidth: 10,
            boxHeight: 10,
            font: {
              size: 12,
              family: fontStack,
            },
          },
        },
      },
    },
  });
});
