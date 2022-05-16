document.addEventListener("DOMContentLoaded", function () {
  const dateRange = document.getElementById("date-range");
  const dateStart = document.getElementById("date-start");
  const dateEnd = document.getElementById("date-end");
  if (!dateRange) return;

  // get the date_range from the current query string date_range
  const url = new URL(window.location.href);
  const date_range = url.searchParams.get("date_range");

  // set the date_range to the current query string date_range
  if (date_range) {
    dateRange.value = date_range;
  }

  // update the url with the new date_range
  dateRange.addEventListener("change", function () {
    // take the value of the date_range, which is in days, and get the
    // date_start and date_end, the date_end is today and the date_start is
    // date_end - date_range
    const date_range = parseInt(dateRange.value);
    let dateEndValue = new Date();
    let dateStartValue = new Date(dateEndValue.getTime() - date_range * 86400000);

    dateEndValue = dateEndValue.toISOString().split("T")[0];
    dateStartValue = dateStartValue.toISOString().split("T")[0];

    dateEnd.value = dateEndValue;
    dateStart.value = dateStartValue;

    // get parent form and submit
    const form = dateRange.closest("form");
    form.submit();
  });

  // if dateStart and dateEnd are both set and and dateStart or dateEnd change
  // then submit the parent form
  dateStart.addEventListener("change", function () {
    if (dateStart.value && dateEnd.value) {
      // if dateStart is greater than dateEnd then swap the values
      if (new Date(dateStart.value) > new Date(dateEnd.value)) {
        const temp = dateStart.value;
        dateStart.value = dateEnd.value;
        dateEnd.value = temp;
      }
      dateRange.value = "custom";
      const form = dateStart.closest("form");
      form.submit();
    }
  });
  dateEnd.addEventListener("change", function () {
    if (dateStart.value && dateEnd.value) {
      // if dateStart is greater than dateEnd then swap the values
      if (new Date(dateStart.value) > new Date(dateEnd.value)) {
        const temp = dateStart.value;
        dateStart.value = dateEnd.value;
        dateEnd.value = temp;
      }
      dateRange.value = "custom";
      const form = dateEnd.closest("form");
      form.submit();
    }
  });
});
