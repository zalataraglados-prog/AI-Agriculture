/**
 * UI & rendering module.
 */
window.UI = (() => {
    let envChart;
    let faultTrendChart;
    let latestImageUploads = [];

    const formatDate = (ts) => {
        if (!ts) return '--';
        const d = new Date(ts);
        if (Number.isNaN(d.getTime())) return ts;
        return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}`;
    };

    const switchView = (viewId, el) => {
        if (el) {
            document.querySelectorAll('.sidebar-item').forEach((item) => item.classList.remove('active'));
            el.classList.add('active');
        }

        document.querySelectorAll('.view-section').forEach((sec) => sec.classList.remove('active'));
        const targetSection = document.getElementById(viewId);
        if (targetSection) {
            targetSection.classList.add('active');
            window.scrollTo({ top: 0, behavior: 'smooth' });
        }

        if (viewId === 'view-home' || viewId === 'view-sensor-detail') {
            if (envChart) envChart.resize();
            if (faultTrendChart) faultTrendChart.resize();
        }

        if (viewId === 'view-charts') {
            Charts.init();
        }
    };

    const renderSensorGrid = (telemetry) => {
        const sensorGrid = document.getElementById('sensorGrid');
        if (!sensorGrid) return;

        const data = Array.isArray(telemetry) ? telemetry : [];
        if (!data.length) {
            sensorGrid.innerHTML = '<div class="col-span-full p-6 text-center text-slate-500 text-xs">暂无遥测数据</div>';
            return;
        }

        const uniqueSensors = Array.from(new Set(data.map((r) => r.sensor_id).filter(Boolean)));
        sensorGrid.innerHTML = uniqueSensors
            .map((sid) => {
                const latest = data.find((r) => r.sensor_id === sid);
                const schema = window.API.getSchema().get(sid);
                const { isFault } = window.API.detectSensorFault(latest);
                const statusColor = isFault ? 'text-rose-400' : 'text-emerald-400';
                const icon = sid.includes('soil') ? 'fa-leaf' : sid.includes('mq') ? 'fa-cloud' : 'fa-microchip';
                const fieldPreview = Object.entries(latest?.fields || {})
                    .slice(0, 2)
                    .map(([field, value]) => {
                        const spec = schema?.fields?.get(field);
                        return `${spec?.label || field}: ${window.API.formatNumeric(value, spec?.unit || '')}`;
                    })
                    .join(' | ');

                return `
                <div class="sensor-tile group" onclick="UI.openSensorDetail('${sid}')">
                    <div class="flex items-start justify-between">
                        <i class="fa ${icon} text-lg ${statusColor} opacity-70"></i>
                        <div class="w-1.5 h-1.5 rounded-full ${isFault ? 'bg-rose-500' : 'bg-emerald-500'} animate-pulse"></div>
                    </div>
                    <div>
                        <p class="text-[10px] text-slate-500 font-mono mb-0.5">${latest?.device_id || '-'}</p>
                        <h3 class="text-xs font-bold text-white tracking-wider">${sid.toUpperCase()}</h3>
                        <p class="text-[10px] text-slate-400 mt-1">${fieldPreview || '无字段数据'}</p>
                    </div>
                    <div class="flex items-center justify-between mt-1 pt-2 border-t border-white/5">
                        <span class="text-[9px] text-slate-400">STATUS: ${isFault ? 'FAULT' : 'ONLINE'}</span>
                        <i class="fa fa-chevron-right text-[10px] text-slate-600 group-hover:translate-x-1 transition-transform"></i>
                    </div>
                </div>`;
            })
            .join('');
    };

    const openSensorDetail = (sid) => {
        switchView('view-sensor-detail');
        const container = document.getElementById('sensorDetailContent');
        if (!container) return;

        const schema = window.API.getSchema().get(sid) || { fields: new Map() };
        const latest = window.API.getLatestBySensor(sid);
        const values = latest?.fields || {};
        const { isFault, reasons } = window.API.detectSensorFault(latest);

        const rows = Array.from(schema.fields.values())
            .map((f) => {
                const display = window.API.formatNumeric(values[f.field], f.unit);
                return `
                    <div class="flex items-center justify-between p-4 bg-white/5 rounded-xl border border-white/5">
                        <div class="flex items-center gap-3">
                            <div class="w-2 h-2 rounded-full ${isFault ? 'bg-rose-400' : 'bg-emerald-400'}"></div>
                            <span class="text-xs text-slate-300 font-bold">${f.label}</span>
                        </div>
                        <div class="text-right">
                            <span class="text-sm font-mono text-white">${display}</span>
                            <span class="text-[10px] text-slate-500 ml-1">${f.unit || ''}</span>
                        </div>
                    </div>
                `;
            })
            .join('');

        container.innerHTML = `
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
                <div class="glass-panel p-8 flex flex-col items-center justify-center relative overflow-hidden">
                    <div class="absolute inset-0 bg-gradient-to-br from-emerald-500/5 to-transparent"></div>
                    <div class="w-48 h-48 rounded-full border-8 border-emerald-500/20 flex items-center justify-center relative shadow-[0_0_50px_rgba(16,185,129,0.1)]">
                        <div class="text-center">
                            <p class="text-[10px] text-emerald-400 font-bold tracking-widest uppercase">Sensor Status</p>
                            <p class="text-5xl font-black ${isFault ? 'text-rose-400' : 'text-white'} tracking-widest">${isFault ? 'FAULT' : 'OK'}</p>
                        </div>
                        <div class="absolute inset-[-12px] border-2 border-dashed border-emerald-400/20 rounded-full animate-[spin_10s_linear_infinite]"></div>
                    </div>
                    <h2 class="mt-8 text-2xl font-black text-white tracking-widest uppercase">${sid}</h2>
                    <p class="text-xs text-slate-400 mt-2 font-mono">LATEST_TS: ${formatDate(latest?.ts)}</p>
                    ${reasons.length ? `<p class="text-[10px] text-rose-400 mt-2">${reasons.join(', ')}</p>` : ''}
                </div>
                <div class="space-y-6">
                    <div class="glass-panel p-6">
                        <h3 class="text-sm font-bold text-slate-200 mb-6 flex items-center gap-2 uppercase tracking-widest"><i class="fa fa-list text-emerald-500"></i>字段明细</h3>
                        <div class="space-y-4">
                            ${rows || '<p class="text-slate-500 italic text-sm">暂无字段定义</p>'}
                        </div>
                    </div>
                    <button onclick="UI.switchView('view-home')" class="w-full py-4 bg-emerald-500/20 border border-emerald-500/30 rounded-xl text-emerald-400 font-bold text-sm hover:bg-emerald-500/30 transition-all flex items-center justify-center gap-2">
                        <i class="fa fa-arrow-left"></i> 返回主页
                    </button>
                </div>
            </div>
        `;
    };

    const renderDiagnosis = (imageUploads) => {
        const aiContainer = document.getElementById('aiDiagnosisContainer');
        if (!aiContainer) return;

        latestImageUploads = Array.isArray(imageUploads) ? imageUploads : [];
        if (!latestImageUploads.length) {
            aiContainer.innerHTML = '<div class="p-8 text-center text-slate-500 italic"><p class="text-xs">暂无图传诊断报告</p></div>';
            return;
        }

        aiContainer.innerHTML = latestImageUploads
            .map((r) => {
                const state = r.upload_status || 'stored';
                const diseaseRate = typeof r.disease_rate === 'number' ? `${(r.disease_rate * 100).toFixed(1)}%` : '-';
                const card =
                    state === 'failed'
                        ? { bg: 'bg-rose-500/10', text: 'text-rose-400', border: 'border-rose-500/20', badge: 'FAILED' }
                        : state === 'inferred'
                            ? { bg: 'bg-emerald-500/10', text: 'text-emerald-400', border: 'border-emerald-500/20', badge: 'INFERRED' }
                            : { bg: 'bg-white/5', text: 'text-slate-400', border: 'border-white/5', badge: 'STORED' };
                const imgUrl = r.upload_id
                    ? `/api/v1/image/file?upload_id=${encodeURIComponent(r.upload_id)}`
                    : (r.saved_path ? `/api/v1/image/file?saved_path=${encodeURIComponent(r.saved_path)}` : '');
                const safeUploadId = `${r.upload_id || ''}`.replace(/'/g, "\\'");
                return `
                    <div class="p-4 border ${card.border} ${card.bg} rounded-xl mb-4">
                        <div class="flex justify-between items-start mb-2">
                            <p class="text-[10px] text-slate-400 font-mono">${formatDate(r.captured_at || r.ts)}</p>
                            <span class="text-[10px] ${card.text} uppercase font-bold">${card.badge}</span>
                        </div>
                        <h4 class="text-sm font-bold text-white mb-2">${r.predicted_class || '处理中'}</h4>
                        <p class="text-[11px] text-slate-300 mb-2">患病率: <span class="${card.text} font-semibold">${diseaseRate}</span></p>
                        <div class="h-28 w-full bg-black/30 rounded-lg overflow-hidden border border-white/10 cursor-pointer" onclick="UI.openImagePreview('${imgUrl}', '${safeUploadId}')">
                            ${imgUrl
                                ? `<img src="${imgUrl}" alt="${safeUploadId}" class="w-full h-full object-cover" onerror="this.parentElement.innerHTML='<div class=&quot;w-full h-full flex items-center justify-center text-xs text-slate-500&quot;>图片不可用</div>';" />`
                                : '<div class="w-full h-full flex items-center justify-center text-xs text-slate-500">无图片路径</div>'}
                        </div>
                    </div>
                `;
            })
            .join('');
    };

    const openImagePreview = (url, title = '') => {
        if (!url) return;
        const img = document.getElementById('modalImage');
        const caption = document.getElementById('modalCaption');
        const fallback = document.getElementById('modalImageFallback');
        if (fallback) {
            fallback.classList.add('hidden');
            fallback.classList.remove('flex');
        }
        if (img) img.src = url;
        if (caption) caption.textContent = title || '图传预览';
        if (typeof window.openModal === 'function') window.openModal();
    };

    const Charts = {
        chartInstances: new Map(),
        selectedSector: 'GLOBAL',

        init: async () => {
            const telemetry = window.API.getTelemetry();
            const deviceIds = Array.from(new Set(telemetry.map((r) => r.device_id).filter(Boolean)));
            const sectors = deviceIds.length ? deviceIds : ['GLOBAL'];
            const sectorList = document.getElementById('sectorList');
            if (sectorList) {
                sectorList.innerHTML = sectors
                    .map((id) => {
                        const active = id === Charts.selectedSector ? 'active' : '';
                        return `
                            <div class="sector-item ${active}" onclick="UI.Charts.setSector('${id}', this)">
                                <div class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></div>
                                <span class="text-xs font-bold text-slate-200">${id}</span>
                            </div>
                        `;
                    })
                    .join('');
            }
            await Charts.refresh();
        },

        setSector: (id, el) => {
            Charts.selectedSector = id;
            document.querySelectorAll('.sector-item').forEach((i) => i.classList.remove('active'));
            if (el) el.classList.add('active');
            Charts.refresh();
        },

        refresh: async () => {
            const hours = parseInt(document.getElementById('timeRangeSelect')?.value || '24', 10);
            const showImages = !!document.getElementById('toggleImages')?.checked;
            const selectedSensorsRaw = Array.from(document.querySelectorAll('#sensorCheckboxes input:checked')).map((i) => i.value);

            const history = await window.API.fetchHistory(Charts.selectedSector === 'GLOBAL' ? '' : Charts.selectedSector, hours, 1000);
            const container = document.getElementById('chartStack');
            if (!container) return;

            Charts.chartInstances.forEach((chart) => chart.destroy());
            Charts.chartInstances.clear();
            container.innerHTML = '';

            const selectedSensors = selectedSensorsRaw.length
                ? selectedSensorsRaw
                : Array.from(new Set(history.map((r) => r.sensor_id).filter(Boolean)));
            selectedSensors.forEach((sid) => {
                const sensorData = history.filter((r) => r.sensor_id === sid);
                if (!sensorData.length) return;
                const schema = window.API.getSchema().get(sid);
                const fieldsToRender = [];
                if (schema?.fields?.size) {
                    schema.fields.forEach((fieldSpec, fieldName) => fieldsToRender.push([fieldName, fieldSpec]));
                } else {
                    const sample = sensorData[0]?.fields || {};
                    Object.keys(sample).forEach((fieldName) => {
                        const n = Number(sample[fieldName]);
                        if (!Number.isFinite(n)) return;
                        fieldsToRender.push([
                            fieldName,
                            { label: fieldName, unit: '', data_type: 'number' },
                        ]);
                    });
                }

                fieldsToRender.forEach(([fieldName, fieldSpec]) => {
                    if (!['number', 'float', 'f32', 'f64', 'u8', 'u16', 'u32', 'i32'].includes(`${fieldSpec.data_type}`.toLowerCase())) return;
                    const canvasId = `canvas-${sid}-${fieldName}`;
                    const vals = sensorData.map((r) => Number(r.fields[fieldName])).filter((v) => Number.isFinite(v));
                    const avg = vals.length ? (vals.reduce((a, b) => a + b, 0) / vals.length).toFixed(2) : '--';

                    const card = document.createElement('div');
                    card.className = 'chart-card';
                    card.innerHTML = `
                        <div class="flex items-center justify-between mb-6">
                            <div>
                                <h4 class="text-xs font-bold text-slate-400 uppercase tracking-widest">${sid} / ${fieldSpec.label}</h4>
                                <p class="text-[10px] text-slate-500 mt-1">真实时间轴</p>
                            </div>
                            <div class="avg-badge">
                                <i class="fa fa-line-chart"></i>
                                AVG: ${avg} ${fieldSpec.unit || ''}
                            </div>
                        </div>
                        <div class="h-64"><canvas id="${canvasId}"></canvas></div>
                    `;
                    container.appendChild(card);

                    const ctx = document.getElementById(canvasId)?.getContext('2d');
                    if (!ctx) return;
                    const grad = ctx.createLinearGradient(0, 0, 0, 250);
                    grad.addColorStop(0, 'rgba(16, 185, 129, 0.3)');
                    grad.addColorStop(1, 'rgba(16, 185, 129, 0)');

                    const chart = new Chart(ctx, {
                        type: 'line',
                        data: {
                            labels: sensorData.map((r) => Charts.formatTime(r.ts)),
                            datasets: [
                                {
                                    label: fieldSpec.label,
                                    data: sensorData.map((r) => {
                                        const n = Number(r.fields[fieldName]);
                                        return Number.isFinite(n) ? n : null;
                                    }),
                                    borderColor: '#10b981',
                                    backgroundColor: grad,
                                    borderWidth: 2,
                                    tension: 0.2,
                                    fill: true,
                                    pointRadius: 1,
                                },
                            ],
                        },
                        options: {
                            responsive: true,
                            maintainAspectRatio: false,
                            parsing: false,
                            plugins: { legend: { display: false } },
                            scales: {
                                x: { grid: { display: false }, ticks: { maxRotation: 0, autoSkip: true, maxTicksLimit: 10, font: { size: 10 } } },
                                y: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { font: { size: 10 } } },
                            },
                        },
                    });

                    Charts.chartInstances.set(canvasId, chart);
                });
            });

            if (showImages) {
                const recent = latestImageUploads.slice(0, 8);
                const visionCard = document.createElement('div');
                visionCard.className = 'chart-card border-blue-500/20';
                visionCard.innerHTML = `
                    <h4 class="text-xs font-bold text-blue-400 uppercase tracking-widest mb-4">图传时间轴（真实图片）</h4>
                    <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                        ${recent
                            .map((item) => {
                                const url = item.upload_id
                                    ? `/api/v1/image/file?upload_id=${encodeURIComponent(item.upload_id)}`
                                    : (item.saved_path ? `/api/v1/image/file?saved_path=${encodeURIComponent(item.saved_path)}` : '');
                                const safeUploadId = `${item.upload_id || ''}`.replace(/'/g, "\\'");
                                return `
                                    <div class="aspect-square bg-white/5 rounded-lg border border-white/5 overflow-hidden relative cursor-pointer" onclick="UI.openImagePreview('${url}', '${safeUploadId}')">
                                        ${url ? `<img src="${url}" class="w-full h-full object-cover" />` : '<div class="w-full h-full flex items-center justify-center text-[10px] text-slate-500">无图</div>'}
                                        <p class="absolute bottom-2 left-2 text-[8px] text-slate-200 bg-black/40 px-1 rounded">${formatDate(item.captured_at || item.ts)}</p>
                                    </div>
                                `;
                            })
                            .join('') || '<p class="text-xs text-slate-500">暂无图传记录</p>'}
                    </div>
                `;
                container.appendChild(visionCard);
            }
        },

        formatTime: (ts) => {
            const d = new Date(ts);
            if (Number.isNaN(d.getTime())) return ts || '--';
            return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}`;
        },
    };

    return {
        formatDate,
        switchView,
        renderSensorGrid,
        renderDiagnosis,
        openSensorDetail,
        openImagePreview,
        Charts,
        setEnvChart: (c) => {
            envChart = c;
        },
        setFaultTrendChart: (c) => {
            faultTrendChart = c;
        },
        setImageUploads: (items) => {
            latestImageUploads = Array.isArray(items) ? items : [];
        },
    };
})();
