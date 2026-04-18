/**
 * UI & Rendering Module
 * Handles DOM updates, charts, and view management.
 */

window.UI = (() => {
    let envChart, faultTrendChart;

    const formatDate = (ts) => {
        if (!ts) return '--';
        const d = new Date(ts);
        if (isNaN(d)) return ts;
        return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2,'0')}`;
    };

    const switchView = (viewId, el) => {
        if (el) {
            document.querySelectorAll('.sidebar-item').forEach(item => item.classList.remove('active'));
            el.classList.add('active');
        }

        document.querySelectorAll('.view-section').forEach(sec => sec.classList.remove('active'));
        const targetSection = document.getElementById(viewId);
        if (targetSection) {
            targetSection.classList.add('active');
            window.scrollTo({ top: 0, behavior: 'smooth' });
        }

        if (viewId === 'view-home' || viewId === 'view-sensor-detail') {
            if(envChart) envChart.resize();
            if(faultTrendChart) faultTrendChart.resize();
        }
    };

    const renderSensorGrid = (telemetry) => {
        const sensorGrid = document.getElementById('sensorGrid');
        if (!sensorGrid) return;

        // If no telemetry, use mock data from API module
        const data = telemetry.length > 0 ? telemetry : window.API.getMockSensors();
        
        // Get unique sensor IDs
        const uniqueSensors = Array.from(new Set(data.map(r => r.sensor_id).filter(id => id)));
        
        sensorGrid.innerHTML = uniqueSensors.map(sid => {
            const latest = data.find(r => r.sensor_id === sid);
            const { isFault } = window.API.detectSensorFault(latest);
            const statusColor = isFault ? 'text-rose-400' : 'text-emerald-400';
            const icon = sid.includes('soil') ? 'fa-leaf' : (sid.includes('mq') ? 'fa-cloud' : 'fa-microchip');
            
            return `
            <div class="sensor-tile group" onclick="UI.openSensorDetail('${sid}')">
                <div class="flex items-start justify-between">
                    <i class="fa ${icon} text-lg ${statusColor} opacity-70"></i>
                    <div class="w-1.5 h-1.5 rounded-full ${isFault ? 'bg-rose-500' : 'bg-emerald-500'} animate-pulse"></div>
                </div>
                <div>
                    <p class="text-[10px] text-slate-500 font-mono mb-0.5">${latest.device_id}</p>
                    <h3 class="text-xs font-bold text-white tracking-wider">${sid.toUpperCase()}</h3>
                </div>
                <div class="flex items-center justify-between mt-1 pt-2 border-t border-white/5">
                    <span class="text-[9px] text-slate-400">STATUS: ${isFault ? 'FAULT' : 'ONLINE'}</span>
                    <i class="fa fa-chevron-right text-[10px] text-slate-600 group-hover:translate-x-1 transition-transform"></i>
                </div>
            </div>`;
        }).join('');
    };

    const openSensorDetail = (sid) => {
        // Change Navigation focus
        switchView('view-sensor-detail');
        
        const container = document.getElementById('sensorDetailContent');
        if (!container) return;

        const schema = window.API.getSchema().get(sid) || { fields: new Map() };
        
        // Premium Layout for Detail View
        container.innerHTML = `
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
                <!-- Left: Big Status Gauge / Visual -->
                <div class="glass-panel p-8 flex flex-col items-center justify-center relative overflow-hidden">
                    <div class="absolute inset-0 bg-gradient-to-br from-emerald-500/5 to-transparent"></div>
                    <div class="w-48 h-48 rounded-full border-8 border-emerald-500/20 flex items-center justify-center relative shadow-[0_0_50px_rgba(16,185,129,0.1)]">
                         <div class="text-center">
                             <p class="text-[10px] text-emerald-400 font-bold tracking-widest uppercase">Deep Scan</p>
                             <p class="text-5xl font-black text-white tracking-widest">OK</p>
                         </div>
                         <!-- Decorative spinning ring -->
                         <div class="absolute inset-[-12px] border-2 border-dashed border-emerald-400/20 rounded-full animate-[spin_10s_linear_infinite]"></div>
                    </div>
                    <h2 class="mt-8 text-2xl font-black text-white tracking-widest uppercase">${sid}</h2>
                    <p class="text-xs text-slate-400 mt-2 font-mono">NODE_HASH: ${Math.random().toString(16).slice(2, 10).toUpperCase()}</p>
                </div>

                <!-- Right: Field Data Deep Dive -->
                <div class="space-y-6">
                    <div class="glass-panel p-6">
                         <h3 class="text-sm font-bold text-slate-200 mb-6 flex items-center gap-2 uppercase tracking-widest"><i class="fa fa-list text-emerald-500"></i>底层寄存器特征向量</h3>
                         <div class="space-y-4">
                             ${Array.from(schema.fields.values()).map(f => `
                                <div class="flex items-center justify-between p-4 bg-white/5 rounded-xl border border-white/5">
                                    <div class="flex items-center gap-3">
                                        <div class="w-2 h-2 rounded-full bg-emerald-400"></div>
                                        <span class="text-xs text-slate-300 font-bold">${f.label}</span>
                                    </div>
                                    <div class="text-right">
                                        <span class="text-sm font-mono text-white">--</span>
                                        <span class="text-[10px] text-slate-500 ml-1">${f.unit}</span>
                                    </div>
                                </div>
                             `).join('') || '<p class="text-slate-500 italic text-sm">暂无注册字段信息</p>'}
                         </div>
                    </div>
                    
                    <button onclick="UI.switchView('view-home')" class="w-full py-4 bg-emerald-500/20 border border-emerald-500/30 rounded-xl text-emerald-400 font-bold text-sm hover:bg-emerald-500/30 transition-all flex items-center justify-center gap-2">
                        <i class="fa fa-arrow-left"></i> 返回主仪表盘
                    </button>
                </div>
            </div>
        `;
    };

    const renderDiagnosis = (imageUploads) => {
        const aiContainer = document.getElementById('aiDiagnosisContainer');
        if (!aiContainer) return;

        if (!imageUploads.length) {
            aiContainer.innerHTML = `<div class="p-8 text-center text-slate-500 italic"><p class="text-xs">暂无图传诊断报告</p></div>`;
            return;
        }

        aiContainer.innerHTML = imageUploads.map(r => {
            const isComplete = r.upload_status === 'inferred' || !!r.predicted_class;
            const pClass = r.predicted_class || '正在推断...';
            const theme = isComplete ? { bg: 'bg-emerald-500/10', text: 'text-emerald-400', border: 'border-emerald-500/20' } : { bg: 'bg-white/5', text: 'text-slate-400', border: 'border-white/5' };
            
            return `
            <div class="p-4 border ${theme.border} ${theme.bg} rounded-xl mb-4">
                <div class="flex justify-between items-start mb-2">
                    <p class="text-[10px] text-slate-400 font-mono">${formatDate(r.captured_at || r.ts)}</p>
                    <span class="text-[10px] ${theme.text} uppercase font-bold">${isComplete ? 'Analysis Done' : 'Processing'}</span>
                </div>
                <h4 class="text-sm font-bold text-white mb-2">${pClass}</h4>
                <div class="h-1 w-full bg-black/30 rounded-full overflow-hidden">
                    <div class="h-full bg-emerald-500 w-[80%]"></div>
                </div>
            </div>`;
        }).join('');
    };

    return {
        formatDate,
        switchView,
        renderSensorGrid,
        renderDiagnosis,
        openSensorDetail,
        setEnvChart: (c) => envChart = c,
        setFaultTrendChart: (c) => faultTrendChart = c
    };
})();
