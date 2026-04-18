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

let envChart;

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

function buildApiUrl(base, queryObj) {
    const u = new URL(base, location.origin);
    Object.entries(queryObj).forEach(([k, v]) => { if (v) u.searchParams.set(k, v); });
    return u.toString();
}

function buildTelemetryStatus(vwc) {
    if(vwc === null) return { t: 'OFFLINE', c: 'text-slate-500 bg-white/5 border border-white/10' };
    if(vwc < 20) return { t: '严重干枯', c: 'text-rose-400 bg-rose-500/10 border border-rose-500/20' };
    if(vwc < 40) return { t: '缺水警告', c: 'text-amber-400 bg-amber-500/10 border border-amber-500/20' };
    if(vwc > 80) return { t: '洪涝风险', c: 'text-blue-400 bg-blue-500/10 border border-blue-500/20' };
    return { t: '完美墒情', c: 'text-emerald-400 bg-emerald-500/10 border border-emerald-500/20' };
}

// 4. Main Update Logic
async function updateData() {
    try {
        const telUrl = buildApiUrl('/api/v1/telemetry', { device_id: deviceId, limit: 100 });
        const imgUrl = buildApiUrl('/api/v1/image/uploads', { device_id: deviceId, limit: 15 });
        const [telemetry, imageUploads] = await Promise.all([
            fetchJson(telUrl).catch(() => []),
            fetchJson(imgUrl).catch(() => [])
        ]);

        // Update KPIs
        const devices = new Set();
        const vwcData = [];
        const tempData = [];
        telemetry.forEach(r => {
            if (r.device_id) devices.add(r.device_id);
            const v = parseNum(r?.fields?.vwc);
            const t = parseNum(r?.fields?.temp_c);
            if (v !== null) vwcData.push(v);
            if (t !== null) tempData.push(t);
        });

        const avg = arr => arr.length ? (arr.reduce((a,b)=>a+b,0)/arr.length) : null;
        const aV = avg(vwcData);
        const aT = avg(tempData);
        
        const inferred = imageUploads.filter(x => x.upload_status === 'inferred').length;
        const rate = imageUploads.length ? Math.round((inferred/imageUploads.length)*100) : null;

        document.getElementById('valDeviceCount').textContent = devices.size || '0';
        document.getElementById('valAvgVwc').textContent = aV !== null ? aV.toFixed(1) : '--';
        document.getElementById('valAvgTemp').textContent = aT !== null ? aT.toFixed(1) : '--';
        document.getElementById('valInferRate').textContent = rate !== null ? `${rate}%` : '--';

        // Update Chart
        const trend = telemetry.slice(0, 15).reverse();
        envChart.data.labels = trend.map(x => formatDate(x.ts));
        envChart.data.datasets[0].data = trend.map(x => parseNum(x?.fields?.vwc));
        envChart.data.datasets[1].data = trend.map(x => parseNum(x?.fields?.temp_c));
        envChart.update();

        // Render Telemetry Table
        const tbody = document.getElementById('telemetryBody');
        if(!telemetry.length) {
            tbody.innerHTML = `<tr><td colspan="5" class="px-6 py-10 text-center text-slate-500"><i class="fa fa-inbox text-3xl mb-3 block opacity-50"></i>网络环境静默，暂无遥测源</td></tr>`;
        } else {
            tbody.innerHTML = telemetry.slice(0, 15).map(r => {
                const v = parseNum(r?.fields?.vwc);
                const t = parseNum(r?.fields?.temp_c);
                const stat = buildTelemetryStatus(v);
                return `
                <tr class="hover:bg-white/[0.04] transition-colors group">
                    <td class="px-6 py-4 whitespace-nowrap text-slate-300 font-mono text-xs">${formatDate(r.ts)}</td>
                    <td class="px-6 py-4 whitespace-nowrap text-blue-400 font-mono text-xs">${r.device_id || 'UNKNOWN'}</td>
                    <td class="px-6 py-4 whitespace-nowrap text-slate-200 font-bold">${v !== null ? v.toFixed(1) : '--'}</td>
                    <td class="px-6 py-4 whitespace-nowrap text-amber-200/80 font-bold">${t !== null ? t.toFixed(1) : '--'}</td>
                    <td class="px-6 py-4 whitespace-nowrap text-right">
                        <span class="px-3 py-1.5 rounded bg-black/20 text-[11px] font-bold tracking-wider ${stat.c} flex inline-flex items-center gap-1 justify-end">
                            <div class="w-1.5 h-1.5 rounded-full ${stat.c.split(' ')[0].replace('text-', 'bg-')}"></div> ${stat.t}
                        </span>
                    </td>
                </tr>`;
            }).join('');
        }

        // Render AI Diagnosis Cards
        const aiContainer = document.getElementById('aiDiagnosisContainer');
        if(!imageUploads.length) {
            aiContainer.innerHTML = `<div class="p-8 text-center text-slate-500 border border-dashed border-white/10 rounded-xl mt-4"><i class="fa fa-camera-retro text-3xl mb-3 opacity-50 block"></i><p class="text-sm">图传队列阻塞或无感知节点</p></div>`;
        } else {
            aiContainer.innerHTML = imageUploads.map(r => {
                const isComplete = r.upload_status === 'inferred';
                const pClass = r.predicted_class || '正在张量推断...';
                const confObj = parseNum(r.confidence);
                const conf = confObj !== null ? (confObj * 100).toFixed(1) : 0;
                
                const theme = isComplete ? (diseaseColors[r.predicted_class] || diseaseColors['default']) : { bg: 'bg-white/5', text: 'text-slate-400', border: 'border-white/10', bar: 'bg-blue-500 animate-pulse', icon: 'fa-cogs' };
                const adviceObj = isComplete ? (adviceAdapter[r.predicted_class] || {txt: '未匹配标准治疗手册。核心阵列请求专家实地勘探。', img: ''}) : {txt: '特征图谱正在进入云端神经网络层提取，请稍候...', img: ''};

                return `
                <div class="p-5 border ${theme.border} ${theme.bg} rounded-xl relative overflow-hidden transition-all duration-300 hover:shadow-[0_0_20px_rgba(0,0,0,0.3)] hover:border-opacity-50 group">
                    <!-- Overlay gradient for depth -->
                    <div class="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent pointer-events-none"></div>
                    
                    <div class="relative z-10 flex justify-between items-start mb-4">
                        <div class="flex items-center gap-3">
                            <div class="w-10 h-10 rounded-lg bg-black/40 flex items-center justify-center border border-white/5 shadow-inner cursor-pointer hover:bg-emerald-500/20 transition-colors" onclick="openModal()" title="点击调取原生光谱图像">
                                <i class="${isComplete ? 'fa fa-camera text-emerald-400' : 'fa fa-spinner fa-spin text-blue-400'} text-sm"></i>
                            </div>
                            <div>
                                <p class="text-[10px] text-slate-400 font-mono tracking-widest">${formatDate(r.captured_at)}</p>
                                <p class="text-sm font-bold text-slate-200 tracking-wide">${r.device_id || 'UNKNOWN'}</p>
                            </div>
                        </div>
                    </div>
                    
                    <div class="relative z-10 mb-5 pl-1">
                        <p class="text-[10px] text-slate-400 uppercase tracking-widest mb-1.5 flex items-center gap-1.5"><i class="fa fa-microscope opacity-70"></i>病理定性</p>
                        <p class="text-xl font-bold ${theme.text} drop-shadow-md tracking-wider flex items-center gap-2">
                           <i class="fa ${theme.icon} opacity-80 text-sm"></i> ${pClass}
                        </p>
                        
                        <div class="mt-3 flex items-center gap-3">
                          <div class="h-2 flex-1 bg-black/40 rounded-full overflow-hidden shadow-inner border border-white/5">
                              <div class="h-full ${theme.bar} rounded-full transition-all duration-1000 relative" style="width: ${conf}%;">
                                 <div class="absolute inset-0 bg-white/20 w-1/2 blur-sm translate-x-[-100%] animate-[shimmer_2s_infinite]"></div>
                              </div>
                          </div>
                          <span class="${theme.text} font-mono text-xs font-bold w-12 text-right tracking-wider">${conf}%</span>
                        </div>
                    </div>
                    
                    <div class="relative z-10 p-4 bg-black/30 rounded-xl border border-white/5 shadow-inner backdrop-blur-md flex flex-col md:flex-row gap-4">
                        <div class="flex-1">
                            <p class="text-[10px] ${theme.text} opacity-90 uppercase tracking-widest mb-2 font-bold flex items-center gap-1.5"><i class="fa fa-stethoscope"></i>智脑策略生成</p>
                            <p class="text-sm text-slate-200 leading-relaxed font-light">${adviceObj.txt}</p>
                        </div>
                        ${adviceObj.img ? `<div class="w-24 h-24 rounded-lg overflow-hidden shrink-0 border border-white/10 shadow-lg relative group/img cursor-pointer" title="请在此替换为您本地的病理图片素材">
                            <img src="${adviceObj.img}" onerror="this.onerror=null;this.src='https://placehold.co/400x400/1e293b/34d399?text=请替换图片';" class="w-full h-full object-cover group-hover/img:scale-110 transition-transform duration-500" alt="病症参考占位图" />
                            <div class="absolute inset-0 bg-emerald-500/20 mix-blend-overlay"></div>
                        </div>` : ''}
                    </div>
                </div>`;
            }).join('');
        }
    } catch(e) {
        console.error(e);
    }
}

// Custom animation injected for the shimmer effect
const styleSheet = document.createElement("style");
styleSheet.innerText = `@keyframes shimmer { 100% { transform: translateX(200%); } }`;
document.head.appendChild(styleSheet);

// 5. App Initialization
window.onload = () => {
    Chart.defaults.color = "rgba(255,255,255,0.4)";
    Chart.defaults.font.family = "Inter";
    
    const ctx = document.getElementById('envChart').getContext('2d');
    
    const gradientVwc = ctx.createLinearGradient(0, 0, 0, 400);
    gradientVwc.addColorStop(0, 'rgba(16, 185, 129, 0.4)');
    gradientVwc.addColorStop(1, 'rgba(16, 185, 129, 0.0)');
    
    const gradientTemp = ctx.createLinearGradient(0, 0, 0, 400);
    gradientTemp.addColorStop(0, 'rgba(245, 158, 11, 0.2)');
    gradientTemp.addColorStop(1, 'rgba(245, 158, 11, 0.0)');

    envChart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: [],
            datasets: [
                {
                    label: '林下/土壤墒情 VWC (%)',
                    data: [],
                    borderColor: '#10b981',
                    backgroundColor: gradientVwc,
                    borderWidth: 2,
                    pointRadius: 4,
                    pointHoverRadius: 6,
                    pointBackgroundColor: '#10b981',
                    pointBorderColor: '#0f172a',
                    tension: 0.4,
                    fill: true,
                    yAxisID: 'y'
                },
                {
                    label: '地表环境温度 (°C)',
                    data: [],
                    borderColor: '#f59e0b',
                    backgroundColor: gradientTemp,
                    borderWidth: 2,
                    borderDash: [4, 4],
                    pointRadius: 4,
                    pointHoverRadius: 6,
                    pointBackgroundColor: '#f59e0b',
                    pointBorderColor: '#0f172a',
                    tension: 0.4,
                    fill: true,
                    yAxisID: 'y1'
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            interaction: { mode: 'index', intersect: false },
            plugins: {
                legend: { position: 'top', align: 'end', labels: { boxWidth: 10, usePointStyle: true, font: {size: 11} } },
                tooltip: { backgroundColor: 'rgba(15, 23, 42, 0.95)', titleColor: '#fff', padding: 14, cornerRadius: 10, borderColor: 'rgba(255,255,255,0.1)', borderWidth: 1 }
            },
            scales: {
                x: { grid: { color: 'rgba(255,255,255,0.03)', drawBorder: false } },
                y: { type: 'linear', display: true, position: 'left', grid: { color: 'rgba(255,255,255,0.03)', drawBorder: false }, min: 0, max: 100 },
                y1: { type: 'linear', display: true, position: 'right', grid: { drawOnChartArea: false }, min: -10, max: 50 }
            }
        }
    });

    updateData();
    setInterval(updateData, 8000);
};
