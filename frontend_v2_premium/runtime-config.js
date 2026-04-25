/**
 * Frontend runtime config (DB-first / real-data only).
 * Override via `window.AI_AG_RUNTIME_CONFIG` before this script loads.
 */
(function initRuntimeConfig() {
    const defaults = {
        telemetry: {
            gatewayStaleMs: 5 * 60 * 1000,
            defaultLimit: 300,
            historyMaxLimit: 1000,
            chartRefreshMs: 15000,
            chartPruneWindowMs: 7 * 24 * 3600 * 1000,
        },
        imageUpload: {
            retries: 2,
            timeoutMs: 45000,
        },
    };

    const deepMerge = (base, extra) => {
        const out = { ...base };
        Object.keys(extra || {}).forEach((key) => {
            const bv = base?.[key];
            const ev = extra[key];
            if (bv && typeof bv === 'object' && !Array.isArray(bv) && ev && typeof ev === 'object' && !Array.isArray(ev)) {
                out[key] = deepMerge(bv, ev);
            } else {
                out[key] = ev;
            }
        });
        return out;
    };

    const merged = deepMerge(defaults, window.AI_AG_RUNTIME_CONFIG || {});
    window.RUNTIME_CONFIG = Object.freeze(merged);
})();

