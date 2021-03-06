/**
 * colector.js
 *
 * Our basic collector script that is added to all pages we want to collect.
 * This sends back the following basic events:
 *
 * - session_start
 * - page_view
 * - click
 * - scroll
 *
 * This will also send back custom events that are triggered by pushing data to
 * the collectorQueue with an event name and then event data that are key-value
 * pairs.
 */

(function() {
  // get the collector cookie as user_id, if it doesn't exist, create it
  // set a collector cookie with a random value and set it to expire in a year
  function set_cookie(name, value, expires) {
    var d = new Date();
    d.setTime(d.getTime() + (expires * 24 * 60 * 60 * 1000));
    expires = "expires=" + d.toUTCString();
    document.cookie = name + "=" + value + "; " + expires + "; path=/";
  }

  function get_cookie(name) {
    name = name + "=";
    var ca = document.cookie.split(";");
    for (var i = 0; i < ca.length; i++) {
      var c = ca[i];
      while (c.charAt(0) == " ") {
        c = c.substring(1);
      }
      if (c.indexOf(name) == 0) {
        return c.substring(name.length, c.length);
      }
    }
    return "";
  }

  var collectorUserId = get_cookie("collectoruserid");
  if (collectorUserId === "") {
    collectorUserId = Math.floor(Math.random() * 1000000000);
    set_cookie("collectoruserid", collectorUserId, 365);
    window.collectorQueue.push({
      collector_id: window.collectorId,
      event: "session_start",
      data: {
        user_id: collectorUserId,
        url: window.location.pathname,
        title: document.title,
        referrer: document.referrer,
        screen_width: window.screen.width,
        screen_height: window.screen.height,
        user_agent: "userAgent" in navigator ? navigator.userAgent : "",
        platform: "userAgentData" in navigator ? navigator.userAgentData.platform : "",
        device: "userAgentData" in navigator ? navigator.userAgentData.mobile ? "Mobile" : "Desktop" : "",
        browser: "userAgentData" in navigator ? navigator.userAgentData.brands[navigator.userAgentData.brands.length - 1].brand : "",
      },
    });
  }

  window.collectorQueue = {
    data: window.collectorQueue || [],
    post: function() {
      for (var i = 0; i < this.data.length; i++) {
        var data = this.data[i];
        if (!data.collectorId) {
          data.collectorId = window.collectorId;
        }
        if (!data.user_id) {
          data.user_id = collectorUserId;
        }
        fetch(window.collectorServer + "/collect/", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(data),
        });
      }
      this.data = [];
    },
    push: function(data) {
      this.data.push(data);
      this.post();
    },
  };

  // parse querystring into an object
  function parse_querystring(querystring) {
    var query = {};
    var pairs = querystring.split("&");
    for (var i = 0; i < pairs.length; i++) {
      var pair = pairs[i].split("=");
      query[decodeURIComponent(pair[0])] = decodeURIComponent(pair[1]);
    }
    return query;
  }

  const query = parse_querystring(window.location.search.substring(1));

  // send a page view event
  window.collectorQueue.push({
    collector_id: window.collectorId,
    event: "page_view",
    data: {
      user_id: collectorUserId,
      url: window.location.pathname,
      title: document.title,
      referrer: document.referrer,
      utm_source: query.utm_source,
      utm_medium: query.utm_medium,
      utm_campaign: query.utm_campaign,
    },
  });

  // send click and auxclick events
  document.addEventListener("click", function (event) {
    window.collectorQueue.push({
      collector_id: window.collectorId,
      event: "click",
      data: {
        user_id: collectorUserId,
        url: window.location.pathname,
        title: document.title,
        x: event.clientX,
        y: event.clientY,
        target: event.target.tagName,
        text: event.target.textContent,
      },
    });
  });

  // send scroll events, but only one per second
  var last_scroll_event = 0;
  window.addEventListener("scroll", function () {
    if (new Date().getTime() - last_scroll_event > 1000) {
      window.collectorQueue.push({
        collector_id: window.collectorId,
        event: "scroll",
        data: {
          user_id: collectorUserId,
          url: window.location.pathname,
          title: document.title,
        },
      });
      last_scroll_event = new Date().getTime();
    }
  });

  // send page_leave events
  // add var for when the page was loaded
  var page_loaded = new Date().getTime();
  window.addEventListener("beforeunload", function () {
    window.collectorQueue.push({
      collector_id: window.collectorId,
      event: "page_leave",
      data: {
        user_id: collectorUserId,
        url: window.location.pathname,
        title: document.title,
        time_on_page: new Date().getTime() - page_loaded,
      },
    });
  });
})();
