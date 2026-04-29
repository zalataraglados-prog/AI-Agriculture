/**
 * App Orchestrator
 * Entry point that initializes modules and manages the data loop.
 */

window.onload = async () => {
    if (!window.API.requireAuthOrRedirect()) {
        return;
    }

    const runtime = window.RUNTIME_CONFIG || {};
    const telemetryCfg = runtime.telemetry || {};
    const refreshMs = Number(telemetryCfg.chartRefreshMs) || 15000;
    const params = new URLSearchParams(window.location.search);
    let activeDeviceId = (params.get('device_id') || localStorage.getItem('device_id') || '').trim();
    if (activeDeviceId) localStorage.setItem('device_id', activeDeviceId);

    const setCtxDeviceLabel = (deviceId) => {
        const ctxTitle = document.getElementById('ctxDevice');
        if (ctxTitle) ctxTitle.textContent = `Node: ${deviceId || 'GLOBAL'}`;
    };
    setCtxDeviceLabel(activeDeviceId);

    setInterval(() => {
        const clock = document.getElementById('clock');
        if (clock) clock.innerText = new Date().toLocaleTimeString('en-US', { hour12: false });
    }, 1000);

    initCharts();

    await window.API.loadSchema();
    window.UI.HomePositioning.init();
    window.UI.Upload.init(activeDeviceId);
    window.UI.AI.init();
    await updateAppLoop(activeDeviceId, setCtxDeviceLabel);

    window.APP = {
        refreshNow: async () => {
            activeDeviceId = (localStorage.getItem('device_id') || activeDeviceId || '').trim();
            await updateAppLoop(activeDeviceId, setCtxDeviceLabel);
        },
    };

    setInterval(() => {
        activeDeviceId = (localStorage.getItem('device_id') || activeDeviceId || '').trim();
        updateAppLoop(activeDeviceId, setCtxDeviceLabel);
    }, refreshMs);

    window.UI.switchView('view-home');
};

function initCharts() {
    Chart.defaults.color = 'rgba(255,255,255,0.4)';
    Chart.defaults.font.family = 'Inter';

    const ctx1 = document.getElementById('envChart');
    if (ctx1) {
        const gradEc = ctx1.getContext('2d').createLinearGradient(0, 0, 0, 300);
        gradEc.addColorStop(0, 'rgba(16, 185, 129, 0.4)');
        gradEc.addColorStop(1, 'rgba(16, 185, 129, 0.0)');

        const chart = new Chart(ctx1, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Soil Fertility EC (uS/cm)',
                    data: [],
                    borderColor: '#10b981',
                    backgroundColor: gradEc,
                    borderWidth: 2,
                    tension: 0.4,
                    fill: true,
                }],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: { legend: { display: false } },
                scales: {
                    x: { grid: { display: false } },
                    y: { grid: { color: 'rgba(255,255,255,0.05)' } },
                },
            },
        });
        window.UI.setEnvChart(chart);
    }
}

async function updateAppLoop(deviceId, setCtxDeviceLabel) {
    try {
        let effectiveDeviceId = (deviceId || '').trim();
        const telemetryQuery = effectiveDeviceId ? { device_id: effectiveDeviceId, limit: 300 } : { limit: 300 };
        const uploadQuery = effectiveDeviceId ? { device_id: effectiveDeviceId, limit: 50 } : { limit: 50 };

        let [telemetry, imageUploads] = await Promise.all([
            window.API.fetchJson(window.API.apiUrl('/api/v1/telemetry', telemetryQuery)).catch(() => []),
            window.API.fetchJson(window.API.apiUrl('/api/v1/image/uploads', uploadQuery)).catch(() => []),
        ]);

        if (effectiveDeviceId && (!Array.isArray(telemetry) || telemetry.length === 0)) {
            const globalTelemetry = await window.API
                .fetchJson(window.API.apiUrl('/api/v1/telemetry', { limit: 300 }))
                .catch(() => []);

            if (Array.isArray(globalTelemetry) && globalTelemetry.length > 0) {
                telemetry = globalTelemetry;
                const fallbackDeviceId = `${globalTelemetry[0]?.device_id || ''}`.trim();
                if (fallbackDeviceId) {
                    effectiveDeviceId = fallbackDeviceId;
                    localStorage.setItem('device_id', fallbackDeviceId);
                    imageUploads = await window.API
                        .fetchJson(window.API.apiUrl('/api/v1/image/uploads', { device_id: fallbackDeviceId, limit: 50 }))
                        .catch(() => imageUploads);
                }
            }
        }

        if (!effectiveDeviceId && Array.isArray(telemetry) && telemetry.length > 0) {
            const inferredDeviceId = `${telemetry[0]?.device_id || ''}`.trim();
            if (inferredDeviceId) {
                effectiveDeviceId = inferredDeviceId;
                localStorage.setItem('device_id', inferredDeviceId);
            }
        }

        if (typeof setCtxDeviceLabel === 'function') {
            setCtxDeviceLabel(effectiveDeviceId);
        }

        window.API.setTelemetry(telemetry);
        window.UI.setImageUploads(imageUploads);
        window.UI.renderSensorGrid(telemetry);
        renderDiagnosis(imageUploads);
    } catch (e) {
        console.error('App Loop Error:', e);
    }
}

function renderDiagnosis(imageUploads) {
    if (window.UI && window.UI.renderDiagnosis) {
        window.UI.renderDiagnosis(imageUploads);
    }
}

window.openModal = () => {
    const m = document.getElementById('imageModal');
    if (m) m.classList.remove('opacity-0', 'pointer-events-none');
};
window.closeModal = () => {
    const m = document.getElementById('imageModal');
    const fallback = document.getElementById('modalImageFallback');
    if (fallback) {
        fallback.classList.add('hidden');
        fallback.classList.remove('flex');
    }
    if (m) m.classList.add('opacity-0', 'pointer-events-none');
};
