var shymini = (function() {
  // Capture script origin at load time (before document.currentScript becomes null)
  var scriptSrc = document.currentScript ? document.currentScript.src : "";
  var scriptOrigin = "";
  try {
    var url = new URL(scriptSrc);
    scriptOrigin = url.origin;
  } catch (e) {
    scriptOrigin = "{{ protocol }}://" + window.location.host;
  }

  return {
  dnt: false,
  idempotency: null,
  heartbeatTaskId: null,
  skipHeartbeat: false,
  loadTimeSent: false,
  sendHeartbeat: function () {
    if (document.hidden || shymini.skipHeartbeat) {
      return;
    }

    shymini.skipHeartbeat = true;

    // Only send loadTime on first request to avoid duplicate hits with same loadTime
    // when server-side idempotency cache expires
    var payload = {
      idempotency: shymini.idempotency,
      referrer: document.referrer,
      location: window.location.href
    };
    if (!shymini.loadTimeSent) {
      payload.loadTime =
        window.performance.timing.domContentLoadedEventEnd -
        window.performance.timing.navigationStart;
    }

    fetch(scriptOrigin + "{{ endpoint }}", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify(payload),
      keepalive: true
    })
    .then(function() {
      shymini.loadTimeSent = true;
      shymini.skipHeartbeat = false;
    })
    .catch(function() {
      shymini.skipHeartbeat = false;
    });
  },
  newPageLoad: function () {
    if (shymini.heartbeatTaskId != null) {
      clearInterval(shymini.heartbeatTaskId);
    }
    shymini.idempotency = Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
    shymini.skipHeartbeat = false;
    shymini.loadTimeSent = false;
    shymini.heartbeatTaskId = setInterval(shymini.sendHeartbeat, {{ heartbeat_frequency }});
    shymini.sendHeartbeat();
  }
};
})();

window.addEventListener("load", shymini.newPageLoad);
{% if !script_inject.is_empty() %}
// The following script is not part of shymini, and was instead
// provided by this site's administrator.
// -- START --
{{ script_inject }}
// -- END --
{% endif %}
