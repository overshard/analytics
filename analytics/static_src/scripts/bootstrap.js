import "bootstrap/js/dist/modal";
import "bootstrap/js/dist/toast";
import "bootstrap/js/dist/collapse";
import "bootstrap/js/dist/alert";
import "bootstrap/js/dist/dropdown";
import Tooltip from "bootstrap/js/dist/tooltip";

import "bootstrap/dist/css/bootstrap.min.css";

document.addEventListener("DOMContentLoaded", function () {
  // enable bootstrap tooltips
  const tooltipTriggerList = document.querySelectorAll(
    '[data-bs-toggle="tooltip"]'
  );
  const tooltipList = [...tooltipTriggerList].map(
    (tooltipTriggerEl) => new Tooltip(tooltipTriggerEl)
  );

  // also make sure tooltips have help cursor
  tooltipTriggerList.forEach((tooltipTriggerEl) => {
    tooltipTriggerEl.style.cursor = "help";
  });
});
