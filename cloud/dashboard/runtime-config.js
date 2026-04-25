/**
 * Dashboard runtime config.
 * Override via `window.AI_AG_DASHBOARD_CONFIG` before this script loads.
 */
(function initDashboardRuntimeConfig() {
  const defaults = {
    telemetry: {
      gatewayStaleMs: 5 * 60 * 1000,
      diseaseThreshold: 0.5,
      refreshMs: 15000,
      telemetryLimit: 300,
      imageLimit: 50,
      telemetryTableRows: 10,
      imageTableRows: 10
    },
    sensors: {
      fertilitySensorId: "soil_modbus_02",
      fertilityField: "ec",
      fertilityUnit: "μS/cm"
    }
  };

  const deepMerge = (base, extra) => {
    const out = { ...base };
    Object.keys(extra || {}).forEach((key) => {
      const bv = base?.[key];
      const ev = extra[key];
      if (
        bv &&
        typeof bv === "object" &&
        !Array.isArray(bv) &&
        ev &&
        typeof ev === "object" &&
        !Array.isArray(ev)
      ) {
        out[key] = deepMerge(bv, ev);
      } else {
        out[key] = ev;
      }
    });
    return out;
  };

  const merged = deepMerge(defaults, window.AI_AG_DASHBOARD_CONFIG || {});
  window.DASHBOARD_CONFIG = Object.freeze(merged);
})();
