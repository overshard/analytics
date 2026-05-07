document.addEventListener("DOMContentLoaded", function () {
  /**
   * If any of the checkboxes in the form "#custom-card-form" are changed then
   * get the data from the form in the format:
   * [{"event": "<field_name>", "value": "<field_value>"}]
   * and send it to the server in the body in JSON format to the URL:
   * const url = new URL(window.location.href) + /custom-cards/
   */
  const form = document.getElementById("custom-card-form");
  if (!form) { return; }
  form.addEventListener("change", function () {
    const formData = new FormData(form);
    const data = [];
    for (const [key, value] of formData.entries()) {
      if (key === "csrfmiddlewaretoken") {
        continue;
      }
      data.push({
        event: key,
        value: value === "on" ? true : false,
      });
    }
    let url = new URL(window.location.href);
    url = url.origin + url.pathname + "cards/";
    fetch(url, {
      method: "POST",
      body: JSON.stringify(data),
      headers: {
        "Content-Type": "application/json",
        "X-CSRFToken": form.querySelector("input[name=csrfmiddlewaretoken]").value,
      },
    }).then(function () {
      window.location.reload();
    });
  });
});
