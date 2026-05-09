// Toggles the property's public flag. The form's `action` attribute holds the
// correct endpoint (`/properties/{id}/public`).
document.addEventListener("DOMContentLoaded", function () {
  const form = document.getElementById("is-public-form");
  if (!form) return;

  form.addEventListener("change", function () {
    fetch(form.action, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
    })
      .then((r) => {
        if (!r.ok) throw new Error(`${form.action} returned HTTP ${r.status}`);
        window.location.reload();
      })
      .catch((err) => {
        console.error("public toggle failed:", err);
      });
  });
});
