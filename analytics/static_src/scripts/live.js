/**
 * live.js
 *
 * Gets the latest event from /properties/events/live/ every second and shows it
 * in a bootstrap 5.2 toast that is dismissed after 3 seconds.
 *
 * The "toast-container" is where we add new toasts.
 *
 * Events have an "event" property for the name and a "created_at" property for
 * the datetime.
 */
import Toast from "bootstrap/js/dist/toast";


document.addEventListener("DOMContentLoaded", function () {
  const url = "/properties/events/live/";
  const toastContainer = document.querySelector(".toast-container");
  let previousEvent = null;

  function showEvent(event) {
    // format created_at to be human readable
    // it's in the format of 2022-05-17T23:34:21.074Z
    const createdAt = new Date(event.created_at);
    const createdAtTime = createdAt.toLocaleTimeString();

    const toastEl = document.createElement("div");
    toastEl.classList.add("toast", "fade", "align-items-center", "bg-primary", "text-white", "border-0");
    toastEl.innerHTML = `
      <div class="d-flex">
        <div class="toast-body">${createdAtTime} — ${event.event}</div>
      </div>
    `;
    toastContainer.appendChild(toastEl);

    const toast = new Toast(toastEl);
    toast.show();
    setTimeout(() => {
      toast.hide();
    }, 3000);
    toastEl.addEventListener("hidden.bs.toast", () => {
      toastEl.remove();
    });
  }

  function getEvent() {
    fetch(url)
      .then((response) => response.json())
      .then((data) => {
        if (data.created_at !== previousEvent) {
          showEvent(data);
          previousEvent = data.created_at;
        }
      })
      .catch(_ => {});
  }

  getEvent();
  setInterval(getEvent, 1000);
});
