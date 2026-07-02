(function () {
  "use strict";

  var BASE64_RE = /^[A-Za-z0-9+/=]+$/;

  function parsePayload() {
    var node = document.getElementById("trace-data");
    return JSON.parse(node.textContent || "{}");
  }

  function redactValue(value, indent) {
    indent = indent || 0;
    var pad = "  ".repeat(indent);
    if (value && typeof value === "object" && value.redacted === true) {
      return pad + "⟨redacted⟩";
    }
    if (Array.isArray(value)) {
      if (!value.length) return "[]";
      return (
        "[\n" +
        value
          .map(function (item) {
            return redactValue(item, indent + 1);
          })
          .join(",\n") +
        "\n" +
        pad +
        "]"
      );
    }
    if (value && typeof value === "object") {
      var keys = Object.keys(value);
      if (!keys.length) return "{}";
      return (
        "{\n" +
        keys
          .map(function (key) {
            return (
              "  ".repeat(indent + 1) +
              JSON.stringify(key) +
              ": " +
              redactValue(value[key], indent + 1)
            );
          })
          .join(",\n") +
        "\n" +
        pad +
        "}"
      );
    }
    return JSON.stringify(value);
  }

  function formatEvent(event) {
    return redactValue(event, 0);
  }

  function groupEvents(events) {
    var groups = [];
    var current = null;
    events.forEach(function (event, index) {
      var name = event.event || "";
      if (name === "command.start") {
        current = {
          command: event.command || "command",
          start: event,
          startIndex: index,
          children: [],
          end: null,
          open: true,
        };
        groups.push({ type: "group", group: current });
        return;
      }
      if (name === "command.end" && current) {
        current.end = event;
        current.open = false;
        current = null;
        return;
      }
      if (current) {
        current.children.push({ event: event, index: index });
      } else {
        groups.push({ type: "event", event: event, index: index });
      }
    });
    return groups;
  }

  function statusLabel(group) {
    if (group.open) return "open · incomplete";
    if (!group.end) return "open · incomplete";
    return group.end.ok ? "ok ✓" : "error ✗";
  }

  function statusClass(group) {
    if (group.open || !group.end) return "open-incomplete";
    return group.end.ok ? "ok" : "err";
  }

  function safeDataUri(uri) {
    if (typeof uri !== "string" || uri.indexOf("data:image/png;base64,") !== 0) {
      return null;
    }
    var payload = uri.slice("data:image/png;base64,".length);
    if (!payload || !BASE64_RE.test(payload)) return null;
    return uri;
  }

  function renderTimeline(container, payload, filterText, onSelect) {
    container.textContent = "";
    var events = payload.events || [];
    var filtered = events.filter(function (event) {
      if (!filterText) return true;
      return String(event.event || "")
        .toLowerCase()
        .includes(filterText.toLowerCase());
    });
    if (!filtered.length) {
      var empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = filterText
        ? "No events match the filter."
        : "No events in this trace.";
      container.appendChild(empty);
      return;
    }
    var groups = groupEvents(filtered);
    groups.forEach(function (item) {
      if (item.type === "event") {
        var row = document.createElement("div");
        row.className = "event-row";
        row.dataset.index = String(item.index);
        var name = document.createElement("div");
        name.className = "event-name";
        name.textContent = item.event.event || "(unknown)";
        var meta = document.createElement("div");
        meta.className = "event-meta";
        meta.textContent = "ts " + (item.event.ts_ms || "?");
        row.appendChild(name);
        row.appendChild(meta);
        row.addEventListener("click", function () {
          onSelect(item.event, item.index);
        });
        container.appendChild(row);
        return;
      }
      var group = item.group;
      var wrap = document.createElement("div");
      wrap.className = "group";
      var header = document.createElement("div");
      header.className = "group-header " + statusClass(group);
      header.textContent = group.command;
      var badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = statusLabel(group);
      if (group.end && group.end.duration_ms != null) {
        badge.textContent += " · " + group.end.duration_ms + "ms";
      }
      header.appendChild(badge);
      wrap.appendChild(header);
      group.children.forEach(function (child) {
        var row = document.createElement("div");
        row.className = "event-row";
        row.dataset.index = String(child.index);
        var name = document.createElement("div");
        name.className = "event-name";
        name.textContent = child.event.event || "(unknown)";
        var meta = document.createElement("div");
        meta.className = "event-meta";
        meta.textContent = "ts " + (child.event.ts_ms || "?");
        row.appendChild(name);
        row.appendChild(meta);
        row.addEventListener("click", function () {
          onSelect(child.event, child.index);
        });
        wrap.appendChild(row);
      });
      container.appendChild(wrap);
    });
  }

  function renderDetail(event, payload, shotsEl, bodyEl, titleEl) {
    titleEl.textContent = event.event || "Detail";
    bodyEl.textContent = formatEvent(event);
    shotsEl.textContent = "";
    if (event.event !== "action.artifacts") return;
    ["screenshot_pre", "screenshot_post"].forEach(function (key) {
      var label = key === "screenshot_pre" ? "Before" : "After";
      var wrap = document.createElement("div");
      wrap.className = "shot-wrap";
      var caption = document.createElement("div");
      caption.className = "shot-label";
      caption.textContent = label;
      wrap.appendChild(caption);
      var rel = event[key];
      if (rel && payload.screenshots && payload.screenshots[rel]) {
        var uri = safeDataUri(payload.screenshots[rel]);
        if (uri) {
          var img = document.createElement("img");
          img.alt = label + " screenshot";
          img.src = uri;
          img.addEventListener("click", function () {
            window.open(uri, "_blank", "noopener");
          });
          wrap.appendChild(img);
        } else {
          var bad = document.createElement("div");
          bad.className = "thumb-missing";
          bad.textContent = "screenshot unavailable";
          wrap.appendChild(bad);
        }
      } else {
        var missing = document.createElement("div");
        missing.className = "thumb-missing";
        missing.textContent = "screenshot unavailable";
        wrap.appendChild(missing);
      }
      shotsEl.appendChild(wrap);
    });
  }

  function renderWarnings(payload) {
    var node = document.getElementById("warnings");
    var lines = [];
    (payload.warnings || []).forEach(function (warning) {
      lines.push(String(warning.kind || "warning") + ": " + String(warning.message || ""));
    });
    if (payload.truncated) {
      lines.push("Timeline truncated to the last " + (payload.returned_events || 0) + " events.");
    }
    if (payload.screenshots_skipped) {
      lines.push(
        "Embedded " +
          (payload.screenshots_embedded || 0) +
          " screenshots; " +
          payload.screenshots_skipped +
          " omitted (budget or missing)."
      );
    }
    if (!lines.length) {
      node.className = "banner hidden";
      node.textContent = "";
      return;
    }
    node.className = "banner";
    node.textContent = lines.join("\n");
  }

  var payload = parsePayload();
  var timeline = document.getElementById("timeline");
  var detailBody = document.getElementById("detail-body");
  var detailTitle = document.getElementById("detail-title");
  var shots = document.getElementById("shots");
  var filter = document.getElementById("filter");
  var meta = document.getElementById("meta");
  meta.textContent =
    "session " +
    (payload.session_id || "?") +
    " · " +
    (payload.returned_events || 0) +
    " / " +
    (payload.total_events || 0) +
    " events";

  renderWarnings(payload);

  function selectEvent(event) {
    renderDetail(event, payload, shots, detailBody, detailTitle);
  }

  function rerender() {
    renderTimeline(timeline, payload, filter.value.trim(), selectEvent);
  }

  filter.addEventListener("input", rerender);
  rerender();
  if ((payload.events || []).length) {
    selectEvent(payload.events[0]);
  }
})();
