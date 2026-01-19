var Shymini = (function() {
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
  sendHeartbeat: function () {
    if (document.hidden || Shymini.skipHeartbeat) {
      return;
    }

    Shymini.skipHeartbeat = true;

    fetch(scriptOrigin + "{{ endpoint }}", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        idempotency: Shymini.idempotency,
        referrer: document.referrer,
        location: window.location.href,
        loadTime:
          window.performance.timing.domContentLoadedEventEnd -
          window.performance.timing.navigationStart
      }),
      keepalive: true
    })
    .then(function() {
      Shymini.skipHeartbeat = false;
    })
    .catch(function() {
      Shymini.skipHeartbeat = false;
    });
  },
  newPageLoad: function () {
    if (Shymini.heartbeatTaskId != null) {
      clearInterval(Shymini.heartbeatTaskId);
    }
    Shymini.idempotency = Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
    Shymini.skipHeartbeat = false;
    Shymini.heartbeatTaskId = setInterval(Shymini.sendHeartbeat, {{ heartbeat_frequency }});
    Shymini.sendHeartbeat();
  }
};
})();

window.addEventListener("load", Shymini.newPageLoad);
{% if !script_inject.is_empty() %}
// The following script is not part of Shymini, and was instead
// provided by this site's administrator.
// -- START --
{{ script_inject }}
// -- END --
{% endif %}
