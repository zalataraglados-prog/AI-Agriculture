// 1. Client Mock/Polyfill for Advice Codes
const adviceAdapter = {
    'HealthyLeaf': { txt: '植株状态极为卓越，叶脉清晰无病变。建议维持现有的水肥一体化网络调度，执行例行无人机遥感巡航。', img: 'assets/reference/HealthyLeaf.png' },
    'Bacterial_Leaf_Blight': { txt: '病理特征呈现典型叶枯病。建议启动应急灌溉降温，并由植保无人机队按 1:500 比例喷洒叶枯唑广谱抗菌液。', img: 'assets/reference/Bacterial_Leaf_Blight.png' },
    'Brown_Spot': { txt: '检测到褐斑真菌群落。当前土壤理化性质可能偏向缺钾，系统建议增加可溶性磷钾复合肥滴灌，辅以多菌灵气溶胶覆盖。', img: 'assets/reference/Brown_Spot.png' },
    'Leaf_Blast': { txt: '稻瘟病潜伏期确认！这是由于过去 48 小时局部高湿引发。系统已触发红黄预警，请立即装载三环唑进行饱和式空中打击。', img: 'assets/reference/Leaf_Blast.png' },
    'Leaf_Scald': { txt: '出现叶尖枯死迹象。建议即刻调低相应地块的水位线进行渗排减湿，晚间安排苯醚甲环唑精准病灶点射。', img: 'assets/reference/Leaf_Scald.png' },
    'Narrow_Brown_Leaf_Spot': { txt: '窄条褐斑蔓延中。该区块微气候通风不畅，请指令智能风机调整角度，同时预备丙环唑药液储备待命。', img: 'assets/reference/Narrow_Brown_Leaf_Spot.png' },
    'Neck_Blast': { txt: '【深红预警】高致病性穗颈瘟萌生危险。如遇抽穗期，此病害将导致全产毁灭。指令控制中心立即安排三环唑预防性全域覆盖。', img: 'assets/reference/Neck_Blast.png' },
    'Rice_Hispa': { txt: '虫害光谱特征吻合铁甲虫啃咬。建议立即启动捕虫灯雷达追踪，如虫口密度越过红线，即刻全覆盖喷洒低毒拟除虫菊酯。', img: 'assets/reference/Rice_Hispa.png' },
};

const diseaseColors = {
    'HealthyLeaf': { bg: 'bg-emerald-500/10', text: 'text-emerald-400', border: 'border-emerald-500/30', bar: 'bg-emerald-500', icon: 'fa-check-circle' },
    'Bacterial_Leaf_Blight': { bg: 'bg-amber-500/10', text: 'text-amber-400', border: 'border-amber-500/30', bar: 'bg-amber-500', icon: 'fa-exclamation-triangle' },
    'Brown_Spot': { bg: 'bg-orange-500/10', text: 'text-orange-400', border: 'border-orange-500/30', bar: 'bg-orange-500', icon: 'fa-bug' },
    'Leaf_Blast': { bg: 'bg-rose-500/10', text: 'text-rose-400', border: 'border-rose-500/30', bar: 'bg-rose-500', icon: 'fa-radiation' },
    'Neck_Blast': { bg: 'bg-red-600/10', text: 'text-red-500', border: 'border-red-600/30', bar: 'bg-red-600', icon: 'fa-skull-crossbones' },
    'default': { bg: 'bg-purple-500/10', text: 'text-purple-400', border: 'border-purple-500/30', bar: 'bg-purple-500', icon: 'fa-search' }
};

// 2. Global Context
setInterval(() => {
    document.getElementById('clock').innerText = new Date().toLocaleTimeString('en-US', {hour12: false});
}, 1000);

const params = new URLSearchParams(location.search);
const deviceId = (params.get('device_id') || localStorage.getItem('device_id') || '').trim();
if (deviceId) localStorage.setItem('device_id', deviceId);
document.getElementById('ctxDevice').textContent = `Node: ${deviceId || 'GLOBAL'}`;

const GATEWAY_STALE_MS = 5 * 60 * 1000;
const DISEASE_THRESHOLD = 0.5;

let envChart;
let faultTrendChart;
let schemaBySensor = new Map();

// Modal Logic
function openModal() {
    const m = document.getElementById('imageModal');
    const c = document.getElementById('modalContent');
    m.classList.remove('opacity-0', 'pointer-events-none');
    c.classList.remove('scale-95');
}
function closeModal() {
    const m = document.getElementById('imageModal');
    const c = document.getElementById('modalContent');
    m.classList.add('opacity-0', 'pointer-events-none');
    c.classList.add('scale-95');
}

// 3. API & Parsing Helpers
function formatDate(ts) {
    if (!ts) return '--';
    const d = new Date(ts);
    if (isNaN(d)) return ts;
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2,'0')}`;
}

function parseNum(v) {
    const n = Number(v); return Number.isFinite(n) ? n : null;
}

async function fetchJson(url) {
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return await res.json();
}

function apiUrl(base, queryObj) {
    const u = new URL(base, location.origin);
    Object.entries(queryObj).forEach(([k, v]) => {
      if (v !== undefined && v !== null && `${v}`.trim() !== '') u.searchParams.set(k, v);
    });
    return u.toString();
}

function fieldSpec(sensorId, field) {
    const sensor = schemaBySensor.get(sensorId);
    return sensor ? (sensor.fields.get(field) || null) : null;
}

function getRequiredFields(sensorId) {
    const sensor = schemaBySensor.get(sensorId);
    if (!sensor) return [];
    return Array.from(sensor.fields.values()).filter(v => v.required).map(v => v.field);
}

function normalizeSchema(payload) {
    const sensors = Array.isArray(payload?.sensors) ? payload.sensors : [];
    sensors.forEach(sensor => {
      if (!sensor?.sensor_id) return;
      const fields = new Map();
      (sensor.fields || []).forEach(field => {
        if (!field?.field) return;
        fields.set(field.field, {
          field: field.field,
          label: field.label || field.field,
          unit: field.unit || '',
          data_type: field.data_type || 'string',
          required: !!field.required,
          threshold_low: typeof field.threshold_low === 'number' ? field.threshold_low : null,
          threshold_high: typeof field.threshold_high === 'number' ? field.threshold_high : null
        });
      });
      schemaBySensor.set(sensor.sensor_id, {
        trendMetric: sensor.trend_metric || null,
        categoryMetric: sensor.category_metric || null,
        fields
      });
    });
}

async function loadSchema() {
    try {
      const schema = await fetchJson('/api/v1/sensor/schema');
      normalizeSchema(schema);
    } catch (err) {
      console.warn("Schema loading failed, falling back to basic checks:", err);
    }
}

function detectSensorFault(record) {
    const sensorId = record.sensor_id;
    const fields = record.fields || {};
    const reasons = [];
    const required = getRequiredFields(sensorId);
    required.forEach(name => {
      if (fields[name] === undefined || fields[name] === null || `${fields[name]}`.trim() === '') {
        reasons.push(`缺少必填字段:${name}`);
      }
    });
  
    const sensorSchema = schemaBySensor.get(sensorId);
    if (sensorSchema) {
      sensorSchema.fields.forEach((spec, field) => {
        const value = fields[field];
        if (value === undefined || value === null || value === '') return;
        if (!['u8', 'u16', 'u32', 'i32', 'f32', 'f64'].includes(spec.data_type)) return;
        const num = parseNum(value);
        if (num === null) { reasons.push(`字段非数值:${field}`); return; }
        if (spec.threshold_low !== null && num < spec.threshold_low) reasons.push(`字段过低:${field}`);
        if (spec.threshold_high !== null && num > spec.threshold_high) reasons.push(`字段过高:${field}`);
      });
    }
    return { isFault: reasons.length > 0, reasons };
}

function buildDeviceLatest(telemetry) {
    const latest = new Map();
    telemetry.forEach(row => {
      const key = row.device_id || 'unknown';
      const old = latest.get(key);
      const ts = Date.parse(row.ts || '');
      if (!old || (Number.isFinite(ts) && ts > old.tsMs)) {
        latest.set(key, { row, tsMs: Number.isFinite(ts) ? ts : -1 });
      }
    });
    return latest;
}

function gatewayFaultDevices(latestMap, nowMs) {
    const out = new Set();
    latestMap.forEach((item, devId) => {
      if (!Number.isFinite(item.tsMs) || nowMs - item.tsMs > GATEWAY_STALE_MS) out.add(devId);
    });
    return out;
}

function sensorFaultDevices(latestMap) {
    const out = new Set();
    latestMap.forEach((item, devId) => {
      if (detectSensorFault(item.row).isFault) out.add(devId);
    });
    return out;
}

function fertilitySeries(telemetry) {
    return telemetry
      .filter(r => r.sensor_id === 'soil_modbus_02')
      .map(r => ({ tsMs: Date.parse(r.ts || ''), ts: r.ts, ec: parseNum(r?.fields?.ec) }))
      .filter(x => Number.isFinite(x.tsMs) && x.ec !== null)
      .sort((a, b) => a.tsMs - b.tsMs);
}

function faultTrendSeries(telemetry, nowMs) {
    const sensorBuckets = new Map();
    const gatewayBuckets = new Map();
    const bucketKey = (tsMs) => {
      const d = new Date(tsMs); d.setSeconds(0, 0); return d.getTime();
    };
  
    telemetry.forEach(row => {
      const tsMs = Date.parse(row.ts || '');
      if (!Number.isFinite(tsMs)) return;
      const key = bucketKey(tsMs);
      if (detectSensorFault(row).isFault) sensorBuckets.set(key, (sensorBuckets.get(key) || 0) + 1);
    });
  
    const byDevice = new Map();
    telemetry.forEach(row => {
      const id = row.device_id || 'unknown';
      if (!byDevice.has(id)) byDevice.set(id, []);
      byDevice.get(id).push(Date.parse(row.ts || ''));
    });
    byDevice.forEach(list => {
      const points = list.filter(Number.isFinite).sort((a, b) => a - b);
      for (let i = 1; i < points.length; i++) {
        if (points[i] - points[i - 1] > GATEWAY_STALE_MS) {
          const key = bucketKey(points[i]);
          gatewayBuckets.set(key, (gatewayBuckets.get(key) || 0) + 1);
        }
      }
      if (points.length && nowMs - points[points.length - 1] > GATEWAY_STALE_MS) {
        gatewayBuckets.set(bucketKey(nowMs), (gatewayBuckets.get(bucketKey(nowMs)) || 0) + 1);
      }
    });
  
    const keys = Array.from(new Set([...sensorBuckets.keys(), ...gatewayBuckets.keys()])).sort((a, b) => a - b);
    return {
      labels: keys.map(k => formatDate(new Date(k).toISOString())),
      sensorFault: keys.map(k => sensorBuckets.get(k) || 0),
      gatewayFault: keys.map(k => gatewayBuckets.get(k) || 0)
    };
}

function fmtRate(v) {
    const n = parseNum(v);
    if (n === null) return null;
    return (n >= 0 && n <= 1) ? n : (n > 1 && n <= 100 ? n/100 : null);
}

// 4. Main Update Logic
window.switchView = function(viewId, el) {
    // 1. Update Navigation UI
    document.querySelectorAll('.sidebar-item').forEach(item => item.classList.remove('active'));
    el.classList.add('active');

    // 2. Switch View Content
    document.querySelectorAll('.view-section').forEach(sec => sec.classList.remove('active'));
    
    const targetSection = document.getElementById(viewId);
    if (targetSection) {
        targetSection.classList.add('active');
    }

    // 3. Accessibility/Focus
    window.scrollTo({ top: 0, behavior: 'smooth' });
    
    // 4. Force Chart Resize if visible (Home/Dashboard view)
    if (viewId === 'view-home') {
        if(envChart) envChart.resize();
        if(faultTrendChart) faultTrendChart.resize();
    }
}

async function updateData() {
    try {
        const telUrl = apiUrl('/api/v1/telemetry', { device_id: deviceId, limit: 300 });
        const imgUrl = apiUrl('/api/v1/image/uploads', { device_id: deviceId, limit: 50 });
        const [telemetry, imageUploads] = await Promise.all([
            fetchJson(telUrl).catch(() => []),
            fetchJson(imgUrl).catch(() => [])
        ]);

        const nowMs = Date.now();
        const latestMap = buildDeviceLatest(telemetry);
        const gatewaySet = gatewayFaultDevices(latestMap, nowMs);
        const sensorFaultSet = sensorFaultDevices(latestMap);
        const faultDeviceSet = new Set([...gatewaySet, ...sensorFaultSet]);

        const soilRows = fertilitySeries(telemetry);
        const avgEc = soilRows.length ? (soilRows.reduce((sum, r) => sum + r.ec, 0) / soilRows.length) : null;

        const diseaseRates = imageUploads.map(r => fmtRate(r.disease_rate)).filter(v => v !== null);
        const avgDiseaseRate = diseaseRates.length ? (diseaseRates.reduce((a, b) => a + b, 0) / diseaseRates.length) : null;

        // Update KPIs
        document.getElementById('valDeviceCount').textContent = latestMap.size || '0';
        document.getElementById('valAvgEc').textContent = avgEc === null ? '--' : `${avgEc.toFixed(1)}`;
        document.getElementById('valFaultDevices').textContent = faultDeviceSet.size || '0';
        document.getElementById('valAvgDiseaseRate').textContent = avgDiseaseRate === null ? '--' : `${(avgDiseaseRate * 100).toFixed(1)}%`;

        // Update Charts
        envChart.data.labels = soilRows.map(r => formatDate(r.ts));
        envChart.data.datasets[0].data = soilRows.map(r => r.ec);
        envChart.update();

        const faultTrend = faultTrendSeries(telemetry, nowMs);
        faultTrendChart.data.labels = faultTrend.labels;
        faultTrendChart.data.datasets[0].data = faultTrend.sensorFault;
        faultTrendChart.data.datasets[1].data = faultTrend.gatewayFault;
        faultTrendChart.update();

        // Render Telemetry Table
        const tbody = document.getElementById('telemetryBody');
        if(!telemetry.length) {
            tbody.innerHTML = `<tr><td colspan="5" class="px-6 py-10 text-center text-slate-500"><i class="fa fa-inbox text-3xl mb-3 block opacity-50"></i>网络环境静默，暂无遥测源</td></tr>`;
        } else {
            const telemetryRows = [...telemetry].sort((a,b) => Date.parse(b.ts||'') - Date.parse(a.ts||'')).slice(0, 15);
            tbody.innerHTML = telemetryRows.map(r => {
                const device = r.device_id || '-';
                const ec = parseNum(r?.fields?.ec);
                const isGatewayFault = gatewaySet.has(device);
                const sensorRes = detectSensorFault(r);
                
                let statusText = '正常';
                let statusCls = 'text-emerald-400 bg-emerald-500/10 border border-emerald-500/20';
                let detail = '数据采集正常';

                if (isGatewayFault) {
                    statusText = '网关掉线';
                    statusCls = 'text-rose-400 bg-rose-500/10 border border-rose-500/20';
                    detail = '5分钟内无心跳信号';
                } else if (sensorRes.isFault) {
                    statusText = '传感器异常';
                    statusCls = 'text-amber-400 bg-amber-500/10 border border-amber-500/20';
                    detail = sensorRes.reasons.join('; ');
                }

                return `
                <tr class="hover:bg-white/[0.04] transition-colors group">
                    <td class="px-6 py-4 whitespace-nowrap text-slate-300 font-mono text-xs">${formatDate(r.ts)}</td>
                    <td class="px-6 py-4 whitespace-nowrap text-blue-400 font-mono text-xs">${device}<span class="ml-1 opacity-40">${r.sensor_id||''}</span></td>
                    <td class="px-6 py-4 whitespace-nowrap text-slate-200 font-bold">${ec !== null ? ec.toFixed(1) : '--'}</td>
                    <td class="px-6 py-4 whitespace-nowrap">
                        <span class="px-2 py-1 rounded text-[10px] font-bold ${statusCls}">${statusText}</span>
                    </td>
                    <td class="px-6 py-4 text-slate-400 text-xs text-right">${detail}</td>
                </tr>`;
            }).join('');
        }

        // Render AI Diagnosis Cards
        const aiContainer = document.getElementById('aiDiagnosisContainer');
        if(!imageUploads.length) {
            aiContainer.innerHTML = `<div class="p-8 text-center text-slate-500 border border-dashed border-white/10 rounded-xl mt-4"><i class="fa fa-camera-retro text-3xl mb-3 opacity-50 block"></i><p class="text-sm">图传队列阻塞或无感知节点</p></div>`;
        } else {
            aiContainer.innerHTML = imageUploads.map(r => {
                const isComplete = r.upload_status === 'inferred' || !!r.predicted_class;
                const pClass = r.predicted_class || '正在张量推断...';
                const dRate = fmtRate(r.disease_rate);
                const conf = dRate !== null ? (dRate * 100).toFixed(1) : (parseNum(r.confidence) ? (r.confidence*100).toFixed(1) : 0);
                
                const theme = isComplete ? (diseaseColors[r.predicted_class] || diseaseColors['default']) : { bg: 'bg-white/5', text: 'text-slate-400', border: 'border-white/10', bar: 'bg-blue-500 animate-pulse', icon: 'fa-cogs' };
                const adviceObj = isComplete ? (adviceAdapter[r.predicted_class] || {txt: '当前状态已记录，模型建议保持观察或人工复核。', img: ''}) : {txt: '边缘端正在回传原始特征图谱，请耐心等待推断结果...', img: ''};

                return `
                <div class="p-5 border ${theme.border} ${theme.bg} rounded-xl relative overflow-hidden transition-all duration-300 hover:shadow-[0_0_20px_rgba(0,0,0,0.3)] hover:border-opacity-50 group">
                    <div class="absolute inset-0 bg-gradient-to-t from-black/20 to-transparent pointer-events-none"></div>
                    <div class="relative z-10 flex justify-between items-start mb-4">
                        <div class="flex items-center gap-3">
                            <div class="w-10 h-10 rounded-lg bg-black/40 flex items-center justify-center border border-white/5 shadow-inner cursor-pointer hover:bg-emerald-500/20 transition-colors" onclick="openModal()">
                                <i class="${isComplete ? 'fa fa-microchip text-emerald-400' : 'fa fa-spinner fa-spin text-blue-400'} text-xs"></i>
                            </div>
                            <div>
                                <p class="text-[10px] text-slate-400 font-mono tracking-widest">${formatDate(r.captured_at || r.ts)}</p>
                                <p class="text-sm font-bold text-slate-200">${r.device_id || 'UNKNOWN'}</p>
                            </div>
                        </div>
                    </div>
                    
                    <div class="relative z-10 mb-5 pl-1">
                        <p class="text-[10px] text-slate-400 uppercase tracking-widest mb-1.5 flex items-center gap-1.5"><i class="fa fa-search opacity-70"></i>病理语义识别</p>
                        <p class="text-xl font-bold ${theme.text} drop-shadow-md tracking-wider flex items-center gap-2">
                           ${pClass}
                        </p>
                        
                        <div class="mt-3 flex items-center gap-3">
                          <div class="h-1.5 flex-1 bg-black/40 rounded-full overflow-hidden border border-white/5">
                              <div class="h-full ${theme.bar} rounded-full transition-all duration-1000" style="width: ${conf}%;"></div>
                          </div>
                          <span class="${theme.text} font-mono text-xs font-bold w-12 text-right">${conf}%</span>
                        </div>
                    </div>
                    
                    <div class="relative z-10 p-3 bg-black/30 rounded-lg border border-white/5 backdrop-blur-md">
                        <p class="text-[10px] ${theme.text} opacity-80 uppercase tracking-widest mb-1.5 font-bold flex items-center gap-1.5"><i class="fa fa-stethoscope"></i>专家处置建议</p>
                        <p class="text-xs text-slate-200 leading-relaxed">${adviceObj.txt}</p>
                    </div>
                </div>`;
            }).join('');
        }
    } catch(e) {
        console.error("Data update failed:", e);
    }
}

// Custom animation injected for the shimmer effect
const styleSheet = document.createElement("style");
styleSheet.innerText = `@keyframes shimmer { 100% { transform: translateX(200%); } }`;
document.head.appendChild(styleSheet);

// 5. App Initialization
window.onload = async () => {
    Chart.defaults.color = "rgba(255,255,255,0.4)";
    Chart.defaults.font.family = "Inter";
    
    // Chart 1: Fertility (EC)
    const ctx1 = document.getElementById('envChart').getContext('2d');
    const gradEc = ctx1.createLinearGradient(0, 0, 0, 300);
    gradEc.addColorStop(0, 'rgba(16, 185, 129, 0.4)');
    gradEc.addColorStop(1, 'rgba(16, 185, 129, 0.0)');

    envChart = new Chart(ctx1, {
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
                x: { grid: { display: false }, ticks: { font: { size: 10 } } },
                y: { grid: { color: 'rgba(255,255,255,0.05)' } }
            }
        }
    });

    // Chart 2: Fault Trend
    const ctx2 = document.getElementById('faultTrendChart').getContext('2d');
    faultTrendChart = new Chart(ctx2, {
        type: 'line',
        data: {
            labels: [],
            datasets: [
                {
                    label: '传感器故障',
                    data: [],
                    borderColor: '#f59e0b',
                    backgroundColor: 'rgba(245, 158, 11, 0.1)',
                    borderWidth: 2,
                    tension: 0.3,
                    fill: true
                },
                {
                    label: '网关掉线',
                    data: [],
                    borderColor: '#ef4444',
                    backgroundColor: 'rgba(239, 68, 68, 0.1)',
                    borderWidth: 2,
                    tension: 0.3,
                    fill: true
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: { legend: { labels: { boxWidth: 10, font: { size: 10 } } } },
            scales: {
                x: { grid: { display: false }, ticks: { font: { size: 10 } } },
                y: { grid: { color: 'rgba(255,255,255,0.05)' }, beginAtZero: true }
            }
        }
    });

    await loadSchema();
    await updateData();
    setInterval(updateData, 15000);

    // Initial view focus
    if(envChart) envChart.resize();
    if(faultTrendChart) faultTrendChart.resize();
};

// -----------------------------------------------------------------------------
// OpenClaw Chat Assistant Integration (Frontend Prototype)
// -----------------------------------------------------------------------------
let isChatOpen = false;
let isAiTyping = false;

window.toggleChat = function() {
    const chatBtn = document.getElementById('chatToggleBtn');
    const chatWindow = document.getElementById('chatWindow');
    const chatIcon = document.getElementById('chatIcon');
    
    isChatOpen = !isChatOpen;

    if (isChatOpen) {
        chatWindow.classList.remove('hidden-chat');
        chatWindow.classList.add('show-chat');
        chatIcon.classList.remove('fa-commenting');
        chatIcon.classList.add('fa-times');
        chatBtn.classList.remove('pulse-glow');
        setTimeout(() => {
            const input = document.getElementById('chatInput');
            if(input) input.focus();
        }, 300);
    } else {
        chatWindow.classList.remove('show-chat');
        chatWindow.classList.add('hidden-chat');
        chatIcon.classList.remove('fa-times');
        chatIcon.classList.add('fa-commenting');
        chatBtn.classList.add('pulse-glow');
    }
}

window.appendChatMsg = function(text, sender) {
    const chatMessages = document.getElementById('chatMessages');
    const msgDiv = document.createElement('div');
    
    if (sender === 'user') {
        msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs ml-auto justify-end';
        msgDiv.innerHTML = `
            <div class="p-3 text-sm rounded-xl msg-user leading-relaxed break-words">
                ${text}
            </div>
        `;
    } else if (sender === 'ai') {
        msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
        msgDiv.innerHTML = `
            <div class="flex-shrink-0 h-8 w-8 rounded-full bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center">
                 <i class="fa fa-paw text-emerald-400 text-xs"></i>
            </div>
            <div class="p-3 text-sm rounded-xl msg-ai leading-relaxed">
                ${text}
            </div>
        `;
    } else if (sender === 'loading') {
        msgDiv.id = 'ai-typing-indicator';
        msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
        msgDiv.innerHTML = `
            <div class="flex-shrink-0 h-8 w-8 rounded-full bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center">
                 <i class="fa fa-paw text-emerald-400 text-xs"></i>
            </div>
            <div class="p-3 text-sm rounded-xl msg-ai flex items-center gap-2">
                <div class="w-2 h-2 bg-emerald-400 rounded-full animate-bounce"></div>
                <div class="w-2 h-2 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
                <div class="w-2 h-2 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
            </div>
        `;
    }
    
    chatMessages.appendChild(msgDiv);
    chatMessages.scrollTop = chatMessages.scrollHeight;
}

window.removeChatLoading = function() {
    const loading = document.getElementById('ai-typing-indicator');
    if (loading) loading.remove();
}

window.handleChatSubmit = async function(e) {
    e.preventDefault();
    if (isAiTyping) return;

    const inputMsg = document.getElementById('chatInput').value.trim();
    if (!inputMsg) return;

    // 1. Render User Message
    window.appendChatMsg(inputMsg, 'user');
    document.getElementById('chatInput').value = '';
    
    // 2. Show Loading Ring
    isAiTyping = true;
    window.appendChatMsg('', 'loading');

    // 3. Call Mock API
    const reply = await window.sendMessageToOpenClaw(inputMsg);
    
    // 4. Render AI Reply
    window.removeChatLoading();
    window.appendChatMsg(reply, 'ai');
    isAiTyping = false;
}

/**
 * 这是一个 API 存根 (Stub)，用于解耦前后端。
 * 当 OpenClaw 后端服务准备就绪时，只需将此函数内部的代码替换为真正的 fetch('/api/v1/chat_proxy', ...) 即可。
 */
window.sendMessageToOpenClaw = async function(message) {
    // 模拟网络延迟
    await new Promise(resolve => setTimeout(resolve, 1500));
    
    // 模拟 AI 响应逻辑
    if (message.includes('肥') || message.includes('肥力') || message.includes('EC')) {
        return `当前广域平均肥力 (EC) 为 ${document.getElementById('valAvgEc').innerText} μS/cm。根据土壤语义模型判定，肥力水平处于正常波动范围。建议维持当前的水肥配比。`;
    }
    if (message.includes('病') || message.includes('稻瘟')) {
        return `我在监控流中注意到了光谱异常。结合智脑策略，如果确诊为稻瘟病潜伏期，请务必立即装载三环唑进行全覆盖。需要我自动下发无人机指令吗？`;
    }
    
    return `收到您的指令：“${message}”。我是 OpenClaw 的前端 UI 原型。目前真正的我还在云端沉睡，等后端工程师把我唤醒后，我就能真的帮您种田了！`;
}
