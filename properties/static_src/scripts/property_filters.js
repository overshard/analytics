/**
 * When clicking a .btn-filter-clear button, get the data-filter-key and the
 * data-filter-value and remove the filter from the URL then reload the page.
 */

document.addEventListener("DOMContentLoaded", function () {
  var filterClearButtons = document.querySelectorAll(".btn-filter-clear");
  for (var i = 0; i < filterClearButtons.length; i++) {
    filterClearButtons[i].addEventListener("click", function (e) {
      var filterKey = e.target.dataset.filterKey;
      var url = new URL(window.location.href);
      url.searchParams.delete(filterKey);
      window.location.href = url.toString();
    });
  }
});
