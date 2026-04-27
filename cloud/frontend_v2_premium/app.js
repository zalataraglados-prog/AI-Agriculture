/**
 * App Orchestrator
 * Entry point that initializes modules and manages the data loop.
 */

window.onload = async () => {
    const runtime = window.RUNTIME_CONFIG || {};
    const telemetryCfg = runtime.telemetry || {};
    const refreshMs = Number(telemetryCfg.chartRefreshMs) || 15000;
    const params = new URLSearchParams(window.location.search);
    let activeDeviceId = (params.get('device_id') || localStorage.getItem('device_id') || '').trim();
    if (activeDeviceId) localStorage.setItem('device_id', activeDeviceId);
    
    const ctxTitle = document.getElementById('ctxDevice');
    if (ctxTitle) ctxTitle.textContent = `Node: ${activeDeviceId || 'GLOBAL'}`;

    // 1. Initialize Global UI Components (Clock)
    setInterval(() => {
        const clock = document.getElementById('clock');
        if (clock) clock.innerText = new Date().toLocaleTimeString('en-US', { hour12: false });
    }, 1000);

    // 2. Initialize Charts
    initCharts();

    // 3. Load Schema & Initial Data
    await window.API.loadSchema();
    window.UI.HomePositioning.init();
    window.UI.Upload.init(activeDeviceId);
    window.UI.AI.init();
    await updateAppLoop(activeDeviceId);

    window.APP = {
        refreshNow: async () => {
            activeDeviceId = (localStorage.getItem('device_id') || activeDeviceId || '').trim();
            await updateAppLoop(activeDeviceId);
        },
    };

    // 4. Start Interval
    setInterval(() => {
        activeDeviceId = (localStorage.getItem('device_id') || activeDeviceId || '').trim();
        updateAppLoop(activeDeviceId);
    }, refreshMs);

    // Initial Resize
    window.UI.switchView('view-home');
};

async function initCharts() {
    Chart.defaults.color = "rgba(255,255,255,0.4)";
    Chart.defaults.font.family = "Inter";
    
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
                    label: '土壤肥力 EC (μS/cm)',
                    data: [],
                    borderColor: '#10b981',
                    backgroundColor: gradEc,
                    borderWidth: 2,
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: { legend: { display: false } },
                scales: {
                    x: { grid: { display: false } },
                    y: { grid: { color: 'rgba(255,255,255,0.05)' } }
                }
            }
        });
        window.UI.setEnvChart(chart);
    }
}

async function updateAppLoop(deviceId) {
    try {
        const telUrl = window.API.apiUrl('/api/v1/telemetry', { device_id: deviceId, limit: 300 });
        const scopedImgUrl = window.API.apiUrl('/api/v1/image/uploads', { device_id: deviceId, limit: 200 });
        const globalImgUrl = window.API.apiUrl('/api/v1/image/uploads', { limit: 200 });

        const [telemetry, scopedImageUploads, globalImageUploads] = await Promise.all([
            window.API.fetchJson(telUrl).catch(() => []),
            window.API.fetchJson(scopedImgUrl).catch(() => []),
            window.API.fetchJson(globalImgUrl).catch(() => [])
        ]);

        const scopedRows = Array.isArray(scopedImageUploads) ? scopedImageUploads : [];
        const globalRows = Array.isArray(globalImageUploads) ? globalImageUploads : [];
        // If device-scoped query returns empty (stale localStorage/query param),
        // fall back to global latest images so "gateway auto-upload" never appears blank.
        const imageUploads = scopedRows.length ? scopedRows : globalRows;

        window.API.setTelemetry(telemetry);
        window.UI.setImageUploads(imageUploads);
        window.UI.renderSensorGrid(telemetry);
        renderDiagnosis(imageUploads);

    } catch (e) {
        console.error("App Loop Error:", e);
    }
}

function renderDiagnosis(imageUploads) {
    if (window.UI && window.UI.renderDiagnosis) {
        window.UI.renderDiagnosis(imageUploads);
    }
}

// Global modal helpers
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
