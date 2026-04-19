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

    const calcYAxisBounds = (values) => {
        if (!values.length) {
            return { min: 0, max: 1 };
        }
        let min = Math.min(...values);
        let max = Math.max(...values);
        if (!Number.isFinite(min) || !Number.isFinite(max)) {
            return { min: 0, max: 1 };
        }
        if (min === max) {
            const pad = Math.max(Math.abs(min) * 0.08, 1);
            return { min: min - pad, max: max + pad };
        }
        const span = max - min;
        const pad = Math.max(span * 0.12, Math.abs(max) * 0.02, 0.5);
        min -= pad;
        max += pad;
        return { min, max };
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

        if (viewId === 'view-health') {
            Health.update();
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
        selectedSector: 'sector-01-a',

        init: async () => {
            // 1. Populate Sectors
            const sectors = [
                { id: 'sector-01-a', name: 'Sector 01-A (Rice)' },
                { id: 'sector-01-b', name: 'Sector 01-B (Corn)' },
                { id: 'sector-02-a', name: 'Sector 02-A (Fruit)' },
                { id: 'sector-02-b', name: 'Sector 02-B (Veg)' }
            ];
            const sectorList = document.getElementById('sectorList');
            if (sectorList) {
                sectorList.innerHTML = sectors.map(s => `
                    <div class="sector-item ${s.id === Charts.selectedSector ? 'active' : ''}" onclick="UI.Charts.setSector('${s.id}', this)">
                        <div class="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]"></div>
                        <span class="text-[11px] font-bold text-slate-300 uppercase tracking-wider">${s.name}</span>
                    </div>
                `).join('');
            }

            // 2. Populate Real Sensors from Schema
            const schema = window.API.getSchema();
            const sensorList = document.getElementById('sensorSelectionList');
            if (sensorList) {
                sensorList.innerHTML = Array.from(schema.keys()).map(sid => `
                    <label class="sensor-pill cursor-pointer group">
                        <input type="checkbox" value="${sid}" class="hidden peer" checked />
                        <i class="fa ${sid.includes('soil') ? 'fa-leaf' : 'fa-microchip'} text-[10px] text-slate-500 peer-checked:text-emerald-400"></i>
                        <span class="text-[10px] text-slate-400 peer-checked:text-emerald-100 uppercase font-bold">${sid}</span>
                    </label>
                `).join('');
            }

            // 3. Set Default Date Range (last 24h)
            const end = new Date();
            const start = new Date(end.getTime() - 24 * 3600 * 1000);
            const toLocalISO = (d) => new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
            
            const startInput = document.getElementById('chartStartTime');
            const endInput = document.getElementById('chartEndTime');
            if (startInput) startInput.value = toLocalISO(start);
            if (endInput) endInput.value = toLocalISO(end);
        },

        setSector: (id, el) => {
            Charts.selectedSector = id;
            document.querySelectorAll('.sector-sidebar .sector-item').forEach(i => i.classList.remove('active'));
            if(el) el.classList.add('active');
            Charts.refresh();
        },

        refresh: async () => {
            const container = document.getElementById('chartStack');
            if (!container) return;

            const startTime = document.getElementById('chartStartTime')?.value;
            const endTime = document.getElementById('chartEndTime')?.value;
            const showImages = document.getElementById('toggleImages')?.checked;
            const selectedSensors = Array.from(document.querySelectorAll('#sensorSelectionList input:checked')).map(i => i.value);

            // Fetch History with explicit range
            const deviceId = localStorage.getItem('device_id') || '';
            container.innerHTML = `<div class="p-20 text-center text-emerald-400 animate-pulse font-mono text-xs">SYNCHRONIZING SECURE TELEMETRY...</div>`;
            
            const history = await window.API.fetchHistory(deviceId, 24, 1000, startTime, endTime);

            // Clear old charts
            Charts.chartInstances.forEach(c => c.destroy());
            Charts.chartInstances.clear();
            container.innerHTML = '';

            if (history.length === 0) {
                container.innerHTML = `<div class="p-20 text-center text-slate-500 italic text-xs">所选时间范围内无历史数据</div>`;
                return;
            }

            // Render selected sensors
            selectedSensors.forEach(sid => {
                const sensorData = history.filter(r => r.sensor_id === sid);
                const schema = window.API.getSchema().get(sid);
                if (!schema || sensorData.length === 0) return;

                schema.fields.forEach((fieldSpec, fieldName) => {
                    const numericTypes = ['number', 'float', 'f32', 'f64', 'u8', 'u16', 'u32', 'i32'];
                    if (!numericTypes.includes(fieldSpec.data_type)) return;
                    
                    const canvasId = `canvas-${sid}-${fieldName}`;
                    
                    // Calculate Average
                    const vals = sensorData.map(r => r.fields[fieldName]).filter(v => typeof v === 'number');
                    const avg = vals.length ? (vals.reduce((a,b) => a+b, 0) / vals.length) : null;

                    const card = document.createElement('div');
                    card.className = 'chart-card';
                    card.innerHTML = `
                        <div class="flex items-center justify-between mb-8">
                            <div class="flex items-center gap-4">
                                <div class="w-10 h-10 rounded-xl bg-white/5 border border-white/10 flex items-center justify-center">
                                    <i class="fa ${sid.includes('soil') ? 'fa-leaf' : 'fa-area-chart'} text-emerald-400"></i>
                                </div>
                                <div>
                                    <h4 class="text-xs font-black text-white uppercase tracking-widest">${sid} / ${fieldSpec.label}</h4>
                                    <p class="text-[9px] text-slate-500 font-mono">HASH: ${btoa(sid+fieldName).slice(0,8)}</p>
                                </div>
                            </div>
                            <div class="avg-badge">
                                <div class="text-[8px] uppercase font-bold text-emerald-500/50 mr-2">Mean Value</div>
                                <span class="text-sm font-black">${avg !== null ? window.API.formatNumeric(avg, fieldSpec.unit) : '--'}</span>
                            </div>
                        </div>
                        <div class="h-72">
                            <canvas id="${canvasId}"></canvas>
                        </div>
                    `;
                    container.appendChild(card);

                    const ctx = document.getElementById(canvasId).getContext('2d');
                    const grad = ctx.createLinearGradient(0, 0, 0, 300);
                    grad.addColorStop(0, 'rgba(255, 255, 255, 0.15)');
                    grad.addColorStop(1, 'rgba(255, 255, 255, 0)');

                    const newChart = new Chart(ctx, {
                        type: 'line',
                        data: {
                            labels: sensorData.map(r => Charts.formatTime(r.ts)),
                            datasets: [{
                                label: fieldSpec.label,
                                data: sensorData.map(r => r.fields[fieldName]),
                                borderColor: '#fff',
                                backgroundColor: grad,
                                borderWidth: 2,
                                tension: 0.4,
                                fill: true,
                                pointRadius: 0,
                                pointHoverRadius: 6,
                                pointHoverBackgroundColor: '#10b981',
                                pointHoverBorderColor: '#fff',
                                pointHoverBorderWidth: 2
                            }]
                        },
                        options: {
                            responsive: true,
                            maintainAspectRatio: false,
                            interaction: { intersect: false, mode: 'index' },
                            plugins: { legend: { display: false } },
                            scales: {
                                x: { grid: { display: false }, ticks: { color: 'rgba(255,255,255,0.3)', font: { size: 9 }, maxRotation: 0, autoSkip: true, maxTicksLimit: 8 } },
                                y: { 
                                    grid: { color: 'rgba(255,255,255,0.05)' }, 
                                    ticks: { color: 'rgba(255,255,255,0.3)', font: { size: 9 } },
                                    suggestedMin: 0
                                }
                            }
                        }
                    });
                    Charts.chartInstances.set(canvasId, newChart);
                });
            });

            // Vision Integration
            if (showImages) {
                const visionCard = document.createElement('div');
                visionCard.className = 'chart-card border-emerald-500/20';
                visionCard.innerHTML = `
                    <div class="flex items-center justify-between mb-6">
                         <h4 class="text-xs font-black text-emerald-400 uppercase tracking-[0.2em] flex items-center gap-2">
                            <i class="fa fa-dot-circle-o"></i> 视觉观测时间轴 (Vision History)
                         </h4>
                    </div>
                    <div id="visionTimeline" class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                        <!-- Simplified mock vision frames matching the timeline -->
                        ${[1,2,3,4,5,6].map(i => `
                            <div class="aspect-[4/3] bg-black/40 rounded-xl border border-white/5 overflow-hidden group relative">
                                <div class="absolute inset-0 flex items-center justify-center">
                                    <i class="fa fa-camera text-white/5 text-4xl group-hover:scale-110 transition-transform"></i>
                                </div>
                                <div class="absolute bottom-2 left-2 right-2 flex justify-between items-center">
                                    <span class="text-[8px] text-white/40 font-mono">FRAME_0${i}</span>
                                    <span class="text-[8px] text-emerald-500/60 font-black">SYNC</span>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                `;
                container.appendChild(visionCard);
            }
        },

        formatTime: (ts) => {
            const d = new Date(ts);
            if (Number.isNaN(d.getTime())) return ts || '--';
            return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
        }
    };

    // --- Health Monitoring Submodule ---
    const Health = {
        update: () => {
            Health.renderServers();
            Health.renderGateways();
            Health.renderSensors();
        },

        renderServers: () => {
            const container = document.getElementById('healthServerList');
            if (!container) return;

            const servers = [
                { name: 'Telemetry Gateway (Rust)', status: 'ok', detail: 'Edge-Cloud 实时链路' },
                { name: 'AI Inference Hub (FastAPI)', status: 'ok', detail: '视觉语义分析引擎' },
                { name: 'Data Persistence (Postgres)', status: 'ok', detail: '时序数据库集群' },
                { name: 'Storage CDN (Object)', status: 'warning', detail: '多媒体分发链路同步滞后' }
            ];

            container.innerHTML = servers.map(s => `
                <div class="status-card health-${s.status} mb-4">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs font-bold text-white">${s.name}</span>
                        <div class="health-dot dot-${s.status}"></div>
                    </div>
                    <p class="text-[10px] text-slate-500 italic">${s.detail}</p>
                </div>
            `).join('');
        },

        renderGateways: () => {
            const container = document.getElementById('healthGatewayList');
            if (!container) return;

            const telemetry = window.API.getTelemetry();
            const deviceIds = Array.from(new Set(telemetry.map(r => r.device_id).filter(id => id)));
            
            if (deviceIds.length === 0) {
                container.innerHTML = '<div class="p-8 text-center text-slate-600 text-[10px] italic">未发现在线网关节点</div>';
                return;
            }

            container.innerHTML = deviceIds.map(id => {
                const latest = telemetry.find(r => r.device_id === id);
                const stale = (Date.now() - new Date(latest.ts).getTime()) > window.API.GATEWAY_STALE_MS;
                const status = stale ? 'critical' : 'ok';
                const statusLabel = stale ? 'OFFLINE / TIMEOUT' : 'CONNECTED / ACTIVE';

                return `
                <div class="status-card health-${status} mb-4">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs font-black text-amber-400 font-mono">${id}</span>
                        <div class="health-dot dot-${status}"></div>
                    </div>
                    <div class="flex justify-between items-center text-[9px] font-bold">
                        <span class="${stale ? 'text-rose-500' : 'text-emerald-500'}">${statusLabel}</span>
                        <span class="text-slate-500 font-mono">${Charts.formatTime(latest.ts)}</span>
                    </div>
                </div>`;
            }).join('');
        },

        renderSensors: () => {
            const container = document.getElementById('healthSensorList');
            if (!container) return;

            const telemetry = window.API.getTelemetry();
            const schema = window.API.getSchema();
            
            const sensorIds = Array.from(schema.keys());
            if (sensorIds.length === 0) {
                container.innerHTML = '<div class="p-8 text-center text-slate-600 text-[10px] italic">等待传感器 Schema 同步...</div>';
                return;
            }

            container.innerHTML = sensorIds.map(sid => {
                const latest = telemetry.find(r => r.sensor_id === sid);
                const { isFault, reasons } = window.API.detectSensorFault(latest);
                const status = !latest ? 'warning' : (isFault ? 'critical' : 'ok');
                const reasonText = reasons.length > 0 ? reasons.join(', ') : (latest ? 'Operational' : 'Waiting for data');

                return `
                <div class="status-card health-${status} mb-4">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs font-bold text-white uppercase">${sid}</span>
                        <div class="health-dot dot-${status}"></div>
                    </div>
                    <div class="flex flex-col gap-1">
                        <p class="text-[9px] text-slate-400 font-medium">${reasonText}</p>
                        <div class="h-1 w-full bg-white/5 rounded-full overflow-hidden mt-1">
                            <div class="h-full ${status === 'ok' ? 'bg-emerald-500' : (status === 'warning' ? 'bg-amber-500' : 'bg-rose-500')} w-[${status === 'ok' ? '100%' : '100%'}]"></div>
                        </div>
                    </div>
                </div>`;
            }).join('');
        }
    };

    return {
        formatDate,
        switchView,
        renderSensorGrid,
        renderDiagnosis,
        openSensorDetail,
        openImagePreview,
        Charts,
        Health,
        setEnvChart: (c) => envChart = c,
        setFaultTrendChart: (c) => faultTrendChart = c,
        setImageUploads: (items) => {
            latestImageUploads = Array.isArray(items) ? items : [];
        },
    };
})();
