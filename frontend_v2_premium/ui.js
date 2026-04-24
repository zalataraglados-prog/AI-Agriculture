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
        const navTargets = document.querySelectorAll('.nav-target');
        navTargets.forEach((item) => item.classList.remove('active'));
        const mappedView = viewId === 'view-sensor-detail' ? 'view-home' : viewId;
        const matched = document.querySelectorAll(`.nav-target[data-view="${mappedView}"]`);
        if (matched.length) {
            matched.forEach((item) => item.classList.add('active'));
        } else if (el) {
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

        if (viewId === 'view-ai') {
            AI.init();
        }
    };

    const renderSchemaNotice = () => {
        const notice = document.getElementById('schemaNotice');
        if (!notice) return;
        if (window.API.isSchemaFallback && window.API.isSchemaFallback()) {
            notice.classList.remove('hidden');
            notice.textContent = 'Schema API unavailable, using fallback mapping (soil_modbus_02 / dht22).';
            return;
        }
        notice.classList.add('hidden');
    };

    const renderSensorGrid = (telemetry) => {
        const sensorGrid = document.getElementById('sensorGrid');
        if (!sensorGrid) return;
        renderSchemaNotice();

        let data = Array.isArray(telemetry) ? telemetry : [];

        const uniqueSensors = Array.from(new Set(data.map((r) => r.sensor_id).filter(Boolean)));
        if (!uniqueSensors.length) {
            sensorGrid.innerHTML = `<div class="col-span-full text-center text-xs text-slate-500 py-8">${window.t('no_data')}</div>`;
            return;
        }
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
                        <p class="text-[10px] text-slate-400 mt-1">${fieldPreview || window.t('no_data')}</p>
                    </div>
                    <div class="flex items-center justify-between mt-1 pt-2 border-t border-white/5">
                        <span class="text-[9px] text-slate-400">STATUS: ${window.t(isFault ? 'status_fault' : 'status_online')}</span>
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
                            <p class="text-[10px] text-emerald-400 font-bold tracking-widest uppercase">${window.t('sensor_status')}</p>
                            <p class="text-5xl font-black ${isFault ? 'text-rose-400' : 'text-white'} tracking-widest">${window.t(isFault ? 'status_fault' : 'status_ok')}</p>
                        </div>
                        <div class="absolute inset-[-12px] border-2 border-dashed border-emerald-400/20 rounded-full animate-[spin_10s_linear_infinite]"></div>
                    </div>
                    <h2 class="mt-8 text-2xl font-black text-white tracking-widest uppercase">${sid}</h2>
                    <p class="text-xs text-slate-400 mt-2 font-mono">LATEST_TS: ${formatDate(latest?.ts)}</p>
                    ${reasons.length ? `<p class="text-[10px] text-rose-400 mt-2">${reasons.join(', ')}</p>` : ''}
                </div>
                <div class="space-y-6">
                    <div class="glass-panel p-6">
                        <h3 class="text-sm font-bold text-slate-200 mb-6 flex items-center gap-2 uppercase tracking-widest"><i class="fa fa-list text-emerald-500"></i>${window.t('field_details')}</h3>
                        <div class="space-y-4">
                            ${rows || '<p class="text-slate-500 italic text-sm">' + window.t('no_data') + '</p>'}
                        </div>
                    </div>
                    <button onclick="UI.switchView('view-home')" class="w-full py-4 bg-emerald-500/20 border border-emerald-500/30 rounded-xl text-emerald-400 font-bold text-sm hover:bg-emerald-500/30 transition-all flex items-center justify-center gap-2">
                        <i class="fa fa-arrow-left"></i> ${window.t('back_home')}
                    </button>
                </div>
            </div>
        `;
    };

    const renderDiagnosis = (imageUploads) => {
        const aiContainer = document.getElementById('aiDiagnosisContainer');
        if (!aiContainer) return;

        let data = Array.isArray(imageUploads) ? imageUploads : [];
        latestImageUploads = data;
        if (!latestImageUploads.length) {
            aiContainer.innerHTML = `<div class="p-6 text-center text-xs text-slate-500">${window.t('no_data')}</div>`;
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
                        <h4 class="text-sm font-bold text-white mb-2">${r.predicted_class || window.t('processing')}</h4>
                        <p class="text-[11px] text-slate-300 mb-2">${window.t('disease_rate')}: <span class="${card.text} font-semibold">${diseaseRate}</span></p>
                        <div class="h-28 w-full bg-black/30 rounded-lg overflow-hidden border border-white/10 cursor-pointer" onclick="UI.openImagePreview('${imgUrl}', '${safeUploadId}')">
                            ${imgUrl
                                ? `<img src="${imgUrl}" alt="${safeUploadId}" class="w-full h-full object-cover" onerror="this.parentElement.innerHTML='<div class=&quot;w-full h-full flex items-center justify-center text-xs text-slate-500&quot;>${window.t('img_fail')}</div>';" />`
                                : '<div class="w-full h-full flex items-center justify-center text-xs text-slate-500">' + window.t('no_data') + '</div>'}
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
        if (caption) caption.textContent = title || window.t('modal_preview');
        if (typeof window.openModal === 'function') window.openModal();
    };

    // --- Home Positioning Submodule ---
    const HomePositioning = {
        selectedCropType: null,
        selectedLocation: null,
        devicesData: null,

        init: async () => {
            const data = await window.API.fetchDevices();
            HomePositioning.devicesData = data;
            HomePositioning.populateDropdown('crop', data.cropTypes);
            HomePositioning.populateDropdown('location', data.locations);
            if (data.cropTypes.length > 0) HomePositioning.selectCrop(data.cropTypes[0]);
            if (data.locations.length > 0) HomePositioning.selectLocation(null);
            HomePositioning.updateSummary();

            // Close dropdowns when clicking outside
            document.addEventListener('click', (e) => {
                if (!e.target.closest('#cropTypeBtn') && !e.target.closest('#cropTypeDropdown')) {
                    document.getElementById('cropTypeDropdown')?.classList.add('hidden');
                }
                if (!e.target.closest('#locationBtn') && !e.target.closest('#locationDropdown')) {
                    document.getElementById('locationDropdown')?.classList.add('hidden');
                }
            });
        },

        populateDropdown: (type, items) => {
            const containerId = type === 'crop' ? 'cropTypeOptions' : 'locationOptions';
            const container = document.getElementById(containerId);
            if (!container) return;
            let html = '';
            if (type === 'location') {
                html += `<div class="px-4 py-2.5 text-[11px] text-slate-300 hover:bg-emerald-500/10 cursor-pointer flex items-center gap-2 transition-colors" onclick="UI.HomePositioning.selectLocation(null)">
                    <i class="fa fa-globe text-blue-400 text-[10px]"></i>
                    <span class="font-bold uppercase tracking-wider">${window.t('all_locations')}</span>
                </div>`;
            }
            items.forEach(item => {
                const icon = type === 'crop' ? 'fa-leaf text-emerald-400' : 'fa-map-pin text-blue-400';
                const fn = type === 'crop' ? 'selectCrop' : 'selectLocation';
                html += `<div class="px-4 py-2.5 text-[11px] text-slate-300 hover:bg-emerald-500/10 cursor-pointer flex items-center gap-2 transition-colors" onclick="UI.HomePositioning.${fn}('${item}')">
                    <i class="fa ${icon} text-[10px]"></i>
                    <span class="font-bold uppercase tracking-wider">${item}</span>
                </div>`;
            });
            container.innerHTML = html;
        },

        toggleDropdown: (type) => {
            const dropdownId = type === 'crop' ? 'cropTypeDropdown' : 'locationDropdown';
            const otherId = type === 'crop' ? 'locationDropdown' : 'cropTypeDropdown';
            document.getElementById(otherId)?.classList.add('hidden');
            document.getElementById(dropdownId)?.classList.toggle('hidden');
        },

        selectCrop: (cropType) => {
            HomePositioning.selectedCropType = cropType;
            const label = document.getElementById('cropTypeBtnLabel');
            if (label) label.textContent = cropType || window.t('crop_select');
            document.getElementById('cropTypeDropdown')?.classList.add('hidden');
            HomePositioning.updateSummary();
        },

        selectLocation: (location) => {
            HomePositioning.selectedLocation = location;
            const label = document.getElementById('locationBtnLabel');
            if (label) label.textContent = location || window.t('all_locations');
            document.getElementById('locationDropdown')?.classList.add('hidden');
            HomePositioning.updateSummary();
        },

        updateSummary: () => {
            const el = document.getElementById('positioningSummary');
            if (!el) return;
            const crop = HomePositioning.selectedCropType || '--';
            const loc = HomePositioning.selectedLocation || window.t('all_locations');
            el.textContent = `ACTIVE > ${crop} / ${loc}`;
        }
    };

    const Upload = {
        selectedFile: null,
        activeDeviceId: '',

        init: (deviceId = '') => {
            Upload.activeDeviceId = (deviceId || localStorage.getItem('device_id') || '').trim();
            const fileInput = document.getElementById('mobileUploadInput');
            const pickBtn = document.getElementById('mobileUploadPickBtn');
            const clearBtn = document.getElementById('mobileUploadClearBtn');
            const submitBtn = document.getElementById('mobileUploadSubmitBtn');
            if (!fileInput || !pickBtn || !submitBtn) return;

            pickBtn.addEventListener('click', () => fileInput.click());
            fileInput.addEventListener('change', () => {
                const [file] = fileInput.files || [];
                Upload.selectedFile = file || null;
                Upload.renderSelectedFile();
            });
            if (clearBtn) {
                clearBtn.addEventListener('click', () => {
                    Upload.selectedFile = null;
                    fileInput.value = '';
                    Upload.setProgress(0);
                    Upload.renderSelectedFile();
                    Upload.setStatus('No image selected.', 'idle');
                });
            }
            submitBtn.addEventListener('click', () => Upload.submit());
            Upload.renderSelectedFile();
            Upload.setStatus('Ready for mobile camera upload.', 'idle');
        },

        setStatus: (message, level = 'idle') => {
            const statusEl = document.getElementById('mobileUploadStatus');
            if (!statusEl) return;
            const palette = {
                idle: 'text-slate-400',
                loading: 'text-emerald-300',
                success: 'text-emerald-400',
                error: 'text-rose-400',
                warn: 'text-amber-300',
            };
            statusEl.className = `text-[11px] ${palette[level] || palette.idle}`;
            statusEl.textContent = message;
        },

        setProgress: (value) => {
            const bar = document.getElementById('mobileUploadProgressBar');
            const label = document.getElementById('mobileUploadProgressText');
            const v = Math.max(0, Math.min(100, Number(value) || 0));
            if (bar) bar.style.width = `${v}%`;
            if (label) label.textContent = `${v}%`;
        },

        renderSelectedFile: () => {
            const nameEl = document.getElementById('mobileUploadFileName');
            const preview = document.getElementById('mobileUploadPreview');
            const emptyState = document.getElementById('mobileUploadPreviewEmpty');
            const submitBtn = document.getElementById('mobileUploadSubmitBtn');
            const file = Upload.selectedFile;
            if (nameEl) {
                if (file) {
                    const mb = (file.size / (1024 * 1024)).toFixed(2);
                    nameEl.textContent = `${file.name} (${mb} MB)`;
                } else {
                    nameEl.textContent = 'No image selected';
                }
            }
            if (submitBtn) submitBtn.disabled = !file;
            if (!preview || !emptyState) return;
            if (!file) {
                preview.src = '';
                preview.classList.add('hidden');
                emptyState.classList.remove('hidden');
                return;
            }
            const blobUrl = URL.createObjectURL(file);
            preview.src = blobUrl;
            preview.onload = () => URL.revokeObjectURL(blobUrl);
            preview.classList.remove('hidden');
            emptyState.classList.add('hidden');
        },

        resolveTag: () => {
            const now = new Date().toISOString();
            let deviceId = (localStorage.getItem('device_id') || Upload.activeDeviceId || '').trim();
            let location = '';
            let cropType = '';
            let farmNote = '';

            const allDevices = HomePositioning.devicesData?.devices || [];
            let pickedDevice = null;
            if (deviceId) {
                pickedDevice = allDevices.find((d) => d.device_id === deviceId) || null;
            }
            if (!pickedDevice) {
                pickedDevice = allDevices.find((d) => {
                    if (HomePositioning.selectedCropType && d.crop_type !== HomePositioning.selectedCropType) return false;
                    if (HomePositioning.selectedLocation && d.location !== HomePositioning.selectedLocation) return false;
                    return true;
                }) || allDevices[0] || null;
            }
            if (pickedDevice) {
                deviceId = pickedDevice.device_id || deviceId;
                location = pickedDevice.location || '';
                cropType = pickedDevice.crop_type || '';
                farmNote = pickedDevice.farm_note || '';
                localStorage.setItem('device_id', deviceId);
            }
            return {
                device_id: deviceId,
                ts: now,
                location,
                crop_type: cropType,
                farm_note: farmNote,
            };
        },

        submit: async () => {
            if (!Upload.selectedFile) {
                Upload.setStatus('Please select an image first.', 'warn');
                return;
            }
            const submitBtn = document.getElementById('mobileUploadSubmitBtn');
            if (submitBtn) submitBtn.disabled = true;
            Upload.setProgress(0);
            const tag = Upload.resolveTag();
            if (!tag.device_id) {
                Upload.setStatus('No device_id found. Open page with ?device_id=... or register device first.', 'error');
                if (submitBtn) submitBtn.disabled = false;
                return;
            }
            Upload.setStatus(`Uploading for ${tag.device_id} ...`, 'loading');
            try {
                const result = await window.API.uploadImage({
                    file: Upload.selectedFile,
                    tag,
                    onProgress: (v) => Upload.setProgress(v),
                });
                Upload.setStatus(`Upload success: ${result.upload_id || 'accepted'}`, 'success');
                if (window.APP && typeof window.APP.refreshNow === 'function') {
                    await window.APP.refreshNow();
                }
            } catch (err) {
                Upload.setStatus(`Upload failed: ${err.message || err}`, 'error');
            } finally {
                if (submitBtn) submitBtn.disabled = !Upload.selectedFile;
            }
        },
    };

    const Charts = {
        chartInstances: new Map(),
        selectedCrop: null,
        selectedLocation: null,
        _devicesData: null,
        _initialized: false,
        _boundDateInputs: false,

        init: async () => {
            // 1. Populate Crop Sectors from API
            const data = await window.API.fetchDevices();
            Charts._devicesData = data;

            // Build crop -> locations map
            const cropLocMap = {};
            data.devices.forEach(d => {
                if (!d.crop_type) return;
                if (!cropLocMap[d.crop_type]) cropLocMap[d.crop_type] = new Set();
                if (d.location) cropLocMap[d.crop_type].add(d.location);
            });

            const sectorList = document.getElementById('sectorList');
            if (sectorList) {
                const cropTypes = Object.keys(cropLocMap);
                if (cropTypes.length > 0 && !Charts.selectedCrop) {
                    Charts.selectedCrop = cropTypes[0];
                    const locs = [...cropLocMap[cropTypes[0]]];
                    if (locs.length > 0) Charts.selectedLocation = locs[0];
                }
                sectorList.innerHTML = cropTypes.map(crop => {
                    const locs = [...cropLocMap[crop]];
                    const isActive = crop === Charts.selectedCrop;
                    return `
                        <div class="sector-crop-group">
                            <div class="sector-item ${isActive ? 'active' : ''}" onclick="UI.Charts.toggleCropLocations('${crop}', this)">
                                <div class="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]"></div>
                                <span class="text-[11px] font-bold text-slate-300 uppercase tracking-wider flex-1">${crop}</span>
                                <i class="fa fa-chevron-${isActive ? 'up' : 'down'} text-[8px] text-slate-500"></i>
                            </div>
                            <div class="sector-locations ${isActive ? '' : 'hidden'}" id="crop-locs-${crop}">
                                ${locs.map(loc => `
                                    <div class="sector-sub-item ${Charts.selectedCrop === crop && Charts.selectedLocation === loc ? 'active' : ''}" onclick="UI.Charts.selectCropLocation('${crop}', '${loc}', this, event)">
                                        <i class="fa fa-map-pin text-[8px] text-blue-400/60"></i>
                                        <span class="text-[10px] text-slate-400 font-bold tracking-wider">${loc}</span>
                                    </div>
                                `).join('')}
                            </div>
                        </div>
                    `;
                }).join('');
            }

            // 2. Populate Real Sensors from Schema
            const schema = window.API.getSchema();
            const sensorList = document.getElementById('sensorSelectionList');
            if (sensorList) {
                const telemetrySensors = Array.from(new Set(window.API.getTelemetry().map((r) => r.sensor_id).filter(Boolean)));
                let sids = Array.from(new Set([...schema.keys(), ...telemetrySensors]));
                sids.sort();
                sensorList.innerHTML = sids.map(sid => `
                    <label class="sensor-pill cursor-pointer group">
                        <input type="checkbox" value="${sid}" class="hidden peer" checked />
                        <i class="fa ${sid.includes('soil') ? 'fa-leaf' : 'fa-microchip'} text-[10px] text-slate-500 peer-checked:text-emerald-400"></i>
                        <span class="text-[10px] text-slate-400 peer-checked:text-emerald-100 uppercase font-bold">${sid}</span>
                    </label>
                `).join('');
            }

            // 3. Set Default Date Range (last 24h) only once, avoid overriding user selection.
            const end = new Date();
            const start = new Date(end.getTime() - 24 * 3600 * 1000);
            const toLocalISO = (d) => new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
            const startInput = document.getElementById('chartStartTime');
            const endInput = document.getElementById('chartEndTime');
            if (startInput) {
                if (!startInput.value) {
                    startInput.value = toLocalISO(start);
                }
            }
            if (endInput) {
                if (!endInput.value) {
                    endInput.value = toLocalISO(end);
                }
            }
            if (!Charts._boundDateInputs) {
                if (startInput) {
                    startInput.addEventListener('change', (e) => {
                        if (endInput) endInput.min = e.target.value;
                    });
                }
                if (endInput) {
                    endInput.addEventListener('change', (e) => {
                        if (startInput) startInput.max = e.target.value;
                    });
                }
                Charts._boundDateInputs = true;
            }
            if (startInput && endInput) {
                endInput.min = startInput.value;
                startInput.max = endInput.value;
            }

            // Global listener to close popover
            if (!Charts._initialized) {
                document.addEventListener('click', (e) => {
                    const popover = document.getElementById('sensorPopover');
                    const btn = document.getElementById('sensorSelectBtn');
                    if (popover && btn && !popover.contains(e.target) && !btn.contains(e.target)) {
                        popover.classList.remove('show-popover');
                    }
                });
            }
            Charts._initialized = true;
        },

        togglePopover: (e) => {
            e.stopPropagation();
            const popover = document.getElementById('sensorPopover');
            if (popover) {
                popover.classList.toggle('show-popover');
            }
        },

        toggleCropLocations: (crop, el) => {
            const locsDiv = document.getElementById(`crop-locs-${crop}`);
            const chevron = el?.querySelector('.fa-chevron-down, .fa-chevron-up');
            if (locsDiv) {
                const isHidden = locsDiv.classList.toggle('hidden');
                if (chevron) {
                    chevron.classList.toggle('fa-chevron-down', isHidden);
                    chevron.classList.toggle('fa-chevron-up', !isHidden);
                }
            }
        },

        selectCropLocation: (crop, location, el, event) => {
            if (event) event.stopPropagation();
            Charts.selectedCrop = crop;
            Charts.selectedLocation = location;
            // Update active states
            document.querySelectorAll('.sector-sidebar .sector-item').forEach(i => i.classList.remove('active'));
            document.querySelectorAll('.sector-sidebar .sector-sub-item').forEach(i => i.classList.remove('active'));
            // Highlight parent crop
            const parentGroup = el?.closest('.sector-crop-group');
            if (parentGroup) parentGroup.querySelector('.sector-item')?.classList.add('active');
            if (el) el.classList.add('active');
            Charts.refresh();
        },

        refresh: async () => {
            const container = document.getElementById('chartStack');
            if (!container) return;

            const startTime = document.getElementById('chartStartTime')?.value;
            const endTime = document.getElementById('chartEndTime')?.value;

            // Date Validation
            if (startTime && endTime && new Date(startTime) > new Date(endTime)) {
                container.innerHTML = `<div class="p-20 text-center text-rose-400 italic text-xs">${window.t('chart_time_invalid') || 'Start time must be before end time'}</div>`;
                return;
            }

            const showImages = document.getElementById('toggleImages')?.checked;
            const selectedSensors = Array.from(document.querySelectorAll('#sensorSelectionList input:checked')).map(i => i.value);

            // Map selected crop/location to ALL actual device IDs,
            // pre-filtering out shadow gateways that were registered long before the query window.
            let deviceIds = [];
            if (Charts._devicesData && Charts._devicesData.devices) {
                const queryEnd = endTime ? new Date(endTime) : new Date();
                // Keep devices registered within 7 days before the end of the query window.
                // This skips obviously stale shadow IDs without affecting real multi-device deployments.
                const PRUNE_WINDOW_MS = 7 * 24 * 3600 * 1000;
                const matchedDevices = Charts._devicesData.devices.filter(d => {
                    if (d.crop_type !== Charts.selectedCrop || d.location !== Charts.selectedLocation) return false;
                    if (!d.registered_at_epoch_sec) return true;
                    const registeredAt = d.registered_at_epoch_sec * 1000;
                    return (queryEnd.getTime() - registeredAt) < PRUNE_WINDOW_MS;
                });
                deviceIds = matchedDevices.map(d => d.device_id);
                // If all were pruned (e.g. user queries very old data), fall back to all matches
                if (deviceIds.length === 0) {
                    deviceIds = Charts._devicesData.devices
                        .filter(d => d.crop_type === Charts.selectedCrop && d.location === Charts.selectedLocation)
                        .map(d => d.device_id);
                }
            }
            if (deviceIds.length === 0) {
                const fallbackId = (localStorage.getItem('device_id') || '').trim();
                if (fallbackId) {
                    deviceIds = [fallbackId];
                }
            }
            if (deviceIds.length === 0) {
                container.innerHTML = `<div class="p-20 text-center text-slate-500 italic text-xs">${window.t('no_data')}</div>`;
                return;
            }

            // Show dynamic progress indicator
            const totalDevices = deviceIds.length;
            container.innerHTML = `<div class="p-20 text-center text-emerald-400 animate-pulse font-mono text-xs">${window.t('syncing')} (0 / ${totalDevices})</div>`;

            let history = [];
            try {
                // Set a timeout to ensure it doesn't hang in case of network issues
                const fetchPromise = window.API.fetchHistory(deviceIds, 24, 1000, startTime, endTime);
                const timeoutPromise = new Promise((_, reject) => setTimeout(() => reject('Timeout'), 10000));
                history = await Promise.race([fetchPromise, timeoutPromise]);
            } catch (err) {
                console.warn('History fetch failed:', err);
            }



            // Clear old charts
            Charts.chartInstances.forEach(c => c.destroy());
            Charts.chartInstances.clear();
            container.innerHTML = '';

            if (history.length === 0) {
                container.innerHTML = `<div class="p-20 text-center text-slate-500 italic text-xs">${window.t('no_history')}</div>`;
                return;
            }

            if (selectedSensors.length === 0) {
                container.innerHTML = `<div class="p-20 text-center text-slate-500 italic text-xs">${window.t('no_data')}</div>`;
                return;
            }

            // Render selected sensors
            selectedSensors.forEach((sid) => {
                const sensorData = history
                    .filter((r) => r.sensor_id === sid)
                    .sort((a, b) => new Date(a.ts || 0).getTime() - new Date(b.ts || 0).getTime());
                const schema = window.API.getSchema().get(sid);
                if (sensorData.length === 0) return;

                let fieldSpecs = [];
                if (schema && schema.fields instanceof Map && schema.fields.size > 0) {
                    fieldSpecs = Array.from(schema.fields.entries()).map(([fieldName, fieldSpec]) => ({
                        fieldName,
                        fieldSpec,
                    }));
                } else {
                    const inferred = new Set();
                    sensorData.forEach((row) => {
                        Object.entries(row?.fields || {}).forEach(([fieldName, value]) => {
                            if (!Number.isFinite(Number(value))) return;
                            inferred.add(fieldName);
                        });
                    });
                    fieldSpecs = Array.from(inferred).map((fieldName) => ({
                        fieldName,
                        fieldSpec: {
                            label: fieldName,
                            unit: '',
                            data_type: 'f64',
                        },
                    }));
                }

                fieldSpecs.forEach(({ fieldName, fieldSpec }) => {
                    const numericTypes = ['number', 'float', 'f32', 'f64', 'u8', 'u16', 'u32', 'u64', 'i32', 'i64'];
                    if (!numericTypes.includes(`${fieldSpec.data_type || ''}`.toLowerCase())) return;

                    const points = sensorData
                        .map((r) => ({
                            label: Charts.formatTime(r.ts),
                            value: Number(r?.fields?.[fieldName]),
                        }))
                        .filter((p) => Number.isFinite(p.value));
                    if (!points.length) return;

                    const values = points.map((p) => p.value);
                    const yBounds = calcYAxisBounds(values);
                    const avg = values.reduce((sum, value) => sum + value, 0) / values.length;
                    const canvasId = `canvas-${sid}-${fieldName}`;

                    const card = document.createElement('div');
                    card.className = 'chart-card';
                    card.innerHTML = `
                        <div class="flex items-center justify-between mb-8">
                            <div class="flex items-center gap-4">
                                <div class="w-10 h-10 rounded-xl bg-white/5 border border-white/10 flex items-center justify-center">
                                    <i class="fa ${sid.includes('soil') ? 'fa-leaf' : 'fa-area-chart'} text-emerald-400"></i>
                                </div>
                                <div>
                                    <h4 class="text-xs font-black text-white uppercase tracking-widest">${sid} / ${fieldSpec.label} ${fieldSpec.unit ? `(${fieldSpec.unit})` : ''}</h4>
                                    <p class="text-[9px] text-slate-500 font-mono">HASH: ${btoa(sid + fieldName).slice(0, 8)}</p>
                                </div>
                            </div>
                            <div class="avg-badge">
                                <div class="text-[8px] uppercase font-bold text-emerald-500/50 mr-2">${window.t('mean_value')}</div>
                                <span class="text-sm font-black">${window.API.formatNumeric(avg, fieldSpec.unit)}</span>
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
                            labels: points.map((p) => p.label),
                            datasets: [{
                                label: fieldSpec.label,
                                data: points.map((p) => p.value),
                                borderColor: '#ffffff',
                                backgroundColor: grad,
                                borderWidth: 2,
                                tension: 0.35,
                                fill: true,
                                pointRadius: 0,
                                pointHoverRadius: 5,
                                pointHoverBackgroundColor: '#10b981',
                                pointHoverBorderColor: '#ffffff',
                                pointHoverBorderWidth: 2
                            }]
                        },
                        options: {
                            responsive: true,
                            maintainAspectRatio: false,
                            interaction: { intersect: false, mode: 'index' },
                            plugins: {
                                legend: { display: false },
                                tooltip: {
                                    displayColors: false,
                                    callbacks: {
                                        label: (context) => {
                                            const val = context.parsed.y;
                                            return `${fieldSpec.label}: ${window.API.formatNumeric(val, fieldSpec.unit)}`;
                                        }
                                    }
                                }
                            },
                            scales: {
                                x: {
                                    grid: { display: false },
                                    ticks: { color: 'rgba(255,255,255,0.3)', font: { size: 9 }, maxRotation: 0, autoSkip: true, maxTicksLimit: 8 }
                                },
                                y: {
                                    grid: { color: 'rgba(255,255,255,0.05)' },
                                    ticks: { color: 'rgba(255,255,255,0.3)', font: { size: 9 } },
                                    min: yBounds.min,
                                    max: yBounds.max
                                }
                            }
                        }
                    });
                    Charts.chartInstances.set(canvasId, newChart);
                });
            });

            // Vision integration (real uploads only, no mock frames)
            if (showImages) {
                const visionCard = document.createElement('div');
                visionCard.className = 'chart-card border-emerald-500/20';
                const startMs = startTime ? new Date(startTime).getTime() : null;
                const endMs = endTime ? new Date(endTime).getTime() + 60 * 1000 : null;
                const visionRows = latestImageUploads
                    .filter((row) => {
                        const ts = new Date(row?.captured_at || row?.received_at || 0).getTime();
                        if (!Number.isFinite(ts)) return false;
                        if (startMs !== null && ts < startMs) return false;
                        if (endMs !== null && ts >= endMs) return false;
                        return true;
                    })
                    .sort((a, b) => new Date(b?.captured_at || b?.received_at || 0).getTime() - new Date(a?.captured_at || a?.received_at || 0).getTime())
                    .slice(0, 12);
                visionCard.innerHTML = `
                    <div class="flex items-center justify-between mb-6">
                         <h4 class="text-xs font-black text-emerald-400 uppercase tracking-[0.2em] flex items-center gap-2">
                            <i class="fa fa-dot-circle-o"></i> ${window.t('vision_timeline')}
                         </h4>
                    </div>
                    <div id="visionTimeline" class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                        ${
                            visionRows.length
                                ? visionRows.map((row) => {
                                    const imgUrl = row.upload_id
                                        ? `/api/v1/image/file?upload_id=${encodeURIComponent(row.upload_id)}`
                                        : (row.saved_path ? `/api/v1/image/file?saved_path=${encodeURIComponent(row.saved_path)}` : '');
                                    const title = `${row.predicted_class || '-'} / ${formatDate(row.captured_at || row.received_at)}`;
                                    return `
                                        <div class="aspect-[4/3] bg-black/40 rounded-xl border border-white/5 overflow-hidden group relative cursor-pointer" onclick="UI.openImagePreview('${imgUrl}', '${title.replace(/'/g, '\\\'')}')">
                                            ${
                                                imgUrl
                                                    ? `<img src="${imgUrl}" alt="${title}" class="w-full h-full object-cover" onerror="this.parentElement.innerHTML='<div class=&quot;w-full h-full flex items-center justify-center text-xs text-slate-500&quot;>${window.t('img_fail')}</div>';" />`
                                                    : `<div class="w-full h-full flex items-center justify-center text-xs text-slate-500">${window.t('no_data')}</div>`
                                            }
                                            <div class="absolute bottom-2 left-2 right-2 flex justify-between items-center bg-black/40 rounded px-1.5 py-0.5">
                                                <span class="text-[8px] text-white/80 font-mono">${formatDate(row.captured_at || row.received_at)}</span>
                                                <span class="text-[8px] text-emerald-300 font-black">${row.upload_status || '-'}</span>
                                            </div>
                                        </div>
                                    `;
                                }).join('')
                                : `<div class="col-span-full text-center text-xs text-slate-500 py-8">${window.t('no_data')}</div>`
                        }
                    </div>
                `;
                container.appendChild(visionCard);
            }
        },

        formatTime: (ts) => {
            const d = new Date(ts);
            if (Number.isNaN(d.getTime())) return ts || '--';

            // Check duration to decide format
            const startTime = document.getElementById('chartStartTime')?.value;
            const endTime = document.getElementById('chartEndTime')?.value;
            let showDate = false;

            if (startTime && endTime) {
                const durationMs = new Date(endTime).getTime() - new Date(startTime).getTime();
                if (durationMs > 26 * 3600 * 1000) { // More than 26h (buffer for timezone/DST)
                    showDate = true;
                }
            }

            const hhmmss = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}`;
            if (showDate) {
                return `${d.getMonth() + 1}-${d.getDate()} ${hhmmss}`;
            }
            return hhmmss;
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

            const telemetry = window.API.getTelemetry();
            const hasTelemetry = telemetry.length > 0;
            const imageRows = Array.isArray(latestImageUploads) ? latestImageUploads : [];
            const inferredCount = imageRows.filter((row) => row?.upload_status === 'inferred').length;
            const failedCount = imageRows.filter((row) => row?.upload_status === 'failed').length;

            const servers = [
                {
                    name: 'Telemetry Gateway (Rust)',
                    status: hasTelemetry ? 'ok' : 'warning',
                    detail: hasTelemetry ? 'Live telemetry packets are flowing.' : 'No live telemetry in current window.'
                },
                {
                    name: 'AI Inference Hub (FastAPI)',
                    status: inferredCount > 0 ? 'ok' : (failedCount > 0 ? 'critical' : 'warning'),
                    detail: inferredCount > 0 ? `Inference completed: ${inferredCount} uploads.` : 'No successful inference in current window.'
                },
                {
                    name: 'Data Persistence (Postgres)',
                    status: hasTelemetry || imageRows.length > 0 ? 'ok' : 'warning',
                    detail: hasTelemetry || imageRows.length > 0 ? 'DB-backed APIs returned real records.' : 'No DB-backed records returned yet.'
                },
                {
                    name: 'Storage (Image File API)',
                    status: imageRows.length > 0 ? 'ok' : 'warning',
                    detail: imageRows.length > 0 ? 'Image uploads are queryable from cloud API.' : 'No recent image upload records.'
                }
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
                container.innerHTML = `<div class="text-xs text-slate-500 p-3">${window.t('no_data')}</div>`;
                return;
            }

            container.innerHTML = deviceIds.map(id => {
                const latest = telemetry.find(r => r.device_id === id);
                const ts = latest ? latest.ts : new Date().toISOString();
                const stale = (Date.now() - new Date(ts).getTime()) > window.API.GATEWAY_STALE_MS;
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
            
            let sensorIds = Array.from(schema.keys());
            if (sensorIds.length === 0) {
                sensorIds = Array.from(new Set(telemetry.map(r => r.sensor_id).filter(Boolean)));
            }
            if (sensorIds.length === 0) {
                container.innerHTML = `<div class="text-xs text-slate-500 p-3">${window.t('no_data')}</div>`;
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
        HomePositioning,
        Upload,
        Charts,
        Health,
        setEnvChart: (c) => envChart = c,
        setFaultTrendChart: (c) => faultTrendChart = c,
        setImageUploads: (items) => {
            latestImageUploads = Array.isArray(items) ? items : [];
        },
        // --- AI Workspace Logic (Multi-session & Instruction Polish for Issue #50) ---
        AI: {
            sessions: [],
            currentSessionId: null,
            instructionList: JSON.parse(localStorage.getItem('agri_ai_instructions') || '[]'),
            tokenCount: parseInt(localStorage.getItem('agri_ai_token_count') || '0'),
            isTyping: false,

            init: () => {
                // 1. Migration for legacy single-history format
                const legacyHistory = localStorage.getItem('agri_ai_history');
                const storedSessions = localStorage.getItem('agri_ai_sessions');
                
                if (storedSessions) {
                    UI.AI.sessions = JSON.parse(storedSessions);
                    UI.AI.currentSessionId = localStorage.getItem('agri_ai_current_session_id');
                } else if (legacyHistory) {
                    // Create first session from legacy data
                    const firstSession = {
                        id: Date.now().toString(),
                        title: '历史会话 (已迁移)',
                        messages: JSON.parse(legacyHistory)
                    };
                    UI.AI.sessions = [firstSession];
                    UI.AI.currentSessionId = firstSession.id;
                    localStorage.removeItem('agri_ai_history');
                }

                // If still no sessions, create a default one
                if (!UI.AI.sessions.length) {
                    UI.AI.createNewSession('新会话');
                } else if (!UI.AI.currentSessionId) {
                    UI.AI.currentSessionId = UI.AI.sessions[0].id;
                }

                UI.AI.renderHistory();
                UI.AI.renderInstructions();
                UI.AI.loadCurrentSession();
                UI.AI.updateTokenUI();
            },

            saveAll: () => {
                localStorage.setItem('agri_ai_sessions', JSON.stringify(UI.AI.sessions));
                localStorage.setItem('agri_ai_current_session_id', UI.AI.currentSessionId);
                localStorage.setItem('agri_ai_instructions', JSON.stringify(UI.AI.instructionList));
            },

            createNewSession: (title = '') => {
                // Prevent creating multiple empty sessions
                const current = UI.AI.sessions.find(s => s.id === UI.AI.currentSessionId);
                if (current && current.messages.length === 0) {
                    alert('已经是最新会话');
                    return;
                }

                const id = Date.now().toString();
                const session = {
                    id: id,
                    title: title || `对话 ${new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`,
                    messages: []
                };
                UI.AI.sessions.unshift(session);
                UI.AI.currentSessionId = id;
                UI.AI.saveAll();
                UI.AI.renderHistory();
                UI.AI.loadCurrentSession();
            },

            switchSession: (id) => {
                UI.AI.currentSessionId = id;
                UI.AI.saveAll();
                UI.AI.renderHistory();
                UI.AI.loadCurrentSession();
            },

            loadCurrentSession: () => {
                const session = UI.AI.sessions.find(s => s.id === UI.AI.currentSessionId);
                if (!session) return;
                
                UI.AI.renderMessagesByList('aiMainMessages', session.messages);
                UI.AI.renderMessagesByList('chatMessages', session.messages);
            },

            renderHistory: () => {
                const container = document.getElementById('aiHistoryList');
                if (!container) return;
                
                container.innerHTML = UI.AI.sessions.map(s => `
                    <div class="ai-history-item ${s.id === UI.AI.currentSessionId ? 'active' : ''}" onclick="UI.AI.switchSession('${s.id}')">
                        <div class="flex items-center justify-between w-full">
                            <div class="flex items-center gap-2 overflow-hidden">
                                <i class="fa fa-comment-o text-xs opacity-50"></i>
                                <span class="text-[11px] font-bold truncate">${s.title}</span>
                            </div>
                            <button onclick="event.stopPropagation(); UI.AI.deleteSession('${s.id}')" class="text-[10px] text-slate-600 hover:text-rose-500 transition-colors">
                                <i class="fa fa-trash-o"></i>
                            </button>
                        </div>
                    </div>
                `).join('');
            },

            renderMessagesByList: (containerId, messages) => {
                const container = document.getElementById(containerId);
                if (!container) return;
                
                if (messages.length === 0 && containerId === 'chatMessages') {
                    container.innerHTML = `
                        <!-- AI Greeting -->
                        <div class="flex w-full mt-2 space-x-3 max-w-xs">
                            <div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5">
                                您好，我已经准备好协助您管理该田块。您可以询问关于云端状态或设备遥控的任何问题。
                            </div>
                        </div>
                    `;
                    return;
                }

                container.innerHTML = messages.map(m => {
                    const isUser = m.role === 'user';
                    const themeClass = containerId === 'aiMainMessages' ? (isUser ? 'msg-user' : 'msg-ai') : (isUser ? 'bg-emerald-600/30' : 'bg-slate-800/80');
                    const bubbleClass = containerId === 'aiMainMessages' ? 'msg-bubble shadow-xl' : 'p-3 rounded-xl border border-white/5';
                    const outerClass = isUser ? 'flex w-full mt-2 space-x-3 ml-auto justify-end' : 'flex w-full mt-2 space-x-3';
                    
                    return `
                        <div class="${outerClass}">
                            <div class="${bubbleClass} ${themeClass} leading-relaxed">
                                ${isUser ? window.CHAT.escapeHtml(m.content).replace(/\n/g, '<br>') : window.CHAT.renderMarkdown(m.content)}
                            </div>
                        </div>
                    `;
                }).join('');
                container.scrollTop = container.scrollHeight;
            },

            addMessage: (role, content) => {
                const session = UI.AI.sessions.find(s => s.id === UI.AI.currentSessionId);
                if (!session) return;
                
                session.messages.push({ role, content, ts: new Date().toISOString() });
                
                // Update session title if it's the first message
                if (session.messages.length === 1 && role === 'user') {
                    session.title = content.length > 15 ? content.substring(0, 15) + '...' : content;
                    UI.AI.renderHistory();
                }

                UI.AI.saveAll();
                UI.AI.tokenCount += Math.ceil(content.length * 1.5);
                UI.AI.updateTokenUI();

                UI.AI.renderMessagesByList('aiMainMessages', session.messages);
                UI.AI.renderMessagesByList('chatMessages', session.messages);
            },

            deleteSession: (id) => {
                if (!confirm('确定要删除此会话吗？')) return;
                UI.AI.sessions = UI.AI.sessions.filter(s => s.id !== id);
                if (UI.AI.currentSessionId === id) {
                    UI.AI.currentSessionId = UI.AI.sessions.length ? UI.AI.sessions[0].id : null;
                }
                if (!UI.AI.sessions.length) UI.AI.createNewSession('新会话');
                UI.AI.saveAll();
                UI.AI.switchSession(UI.AI.currentSessionId);
            },

            clearHistory: () => {
                if (!confirm('这将永久清除所有会话记录，确定吗？')) return;
                UI.AI.sessions = [];
                UI.AI.currentSessionId = null;
                UI.AI.createNewSession('新会话');
            },

            // --- Instruction Management ---
            renderInstructions: () => {
                const container = document.getElementById('aiInstructionList');
                if (!container) return;
                
                if (!UI.AI.instructionList.length) {
                    container.innerHTML = '<p class="text-[10px] text-slate-600 italic p-2">暂无自定义指令</p>';
                    return;
                }

                container.innerHTML = UI.AI.instructionList.map((instr, idx) => `
                    <div class="instruction-item flex items-start gap-2 group">
                        <span class="flex-1 text-[11px] text-slate-400 line-clamp-2 leading-tight">${instr}</span>
                        <button onclick="UI.AI.removeInstruction(${idx})" class="remove-btn text-[10px] text-rose-500/50 hover:text-rose-500">
                            <i class="fa fa-times-circle"></i>
                        </button>
                    </div>
                `).join('');
            },

            addInstruction: () => {
                const input = document.getElementById('aiInstrInput');
                const text = input.value.trim();
                if (!text) return;
                
                UI.AI.instructionList.push(text);
                input.value = '';
                UI.AI.saveAll();
                UI.AI.renderInstructions();
            },

            removeInstruction: (idx) => {
                UI.AI.instructionList.splice(idx, 1);
                UI.AI.saveAll();
                UI.AI.renderInstructions();
            },

            // --- Skill Preview Modal ---
            showSkillPreview: async () => {
                const modal = document.getElementById('aiSkillModal');
                const content = document.getElementById('aiSkillMarkdown');
                if (!modal || !content) return;
                
                modal.classList.add('show-modal');
                content.innerHTML = '<p class="animate-pulse">Loading protocol...</p>';
                
                try {
                    const response = await fetch('AI-ag-agent-skill.md');
                    if (!response.ok) throw new Error('File not found');
                    const text = await response.text();
                    content.innerHTML = window.CHAT.renderMarkdown(text);
                } catch (err) {
                    content.innerHTML = `<p class="text-rose-400">无法加载协议文件: ${err.message}. <br> 请确保根目录下存在 AI-ag-agent-skill.md</p>`;
                }
            },

            hideSkillPreview: () => {
                const modal = document.getElementById('aiSkillModal');
                if (modal) modal.classList.remove('show-modal');
            },

            handleMainSubmit: async (e) => {
                if (e) e.preventDefault();
                if (UI.AI.isTyping) return;

                const input = document.getElementById('aiMainInput');
                const msg = input.value.trim();
                if (!msg) return;

                UI.AI.addMessage('user', msg);
                input.value = '';

                UI.AI.isTyping = true;
                UI.AI.showLoading();

                try {
                    // Combine all active instructions
                    const stack = UI.AI.instructionList.join('\n');
                    const fullPrompt = stack ? `${stack}\n\nClient Input: ${msg}` : msg;
                    const reply = await window.CHAT.sendMessageToOpenClaw(fullPrompt);
                    UI.AI.hideLoading();
                    UI.AI.addMessage('ai', reply);
                } catch (err) {
                    UI.AI.hideLoading();
                    UI.AI.addMessage('ai', `服务暂时离线: ${err.message}`);
                } finally {
                    UI.AI.isTyping = false;
                }
            },

            updateTokenUI: () => {
                const countEl = document.getElementById('tokenCount');
                const barEl = document.getElementById('tokenBar');
                if (countEl) countEl.textContent = UI.AI.tokenCount.toLocaleString();
                if (barEl) {
                    const percent = Math.min((UI.AI.tokenCount / 500000) * 100, 100);
                    barEl.style.width = `${percent}%`;
                }
                localStorage.setItem('agri_ai_token_count', UI.AI.tokenCount);
            },

            showLoading: () => {
                ['aiMainMessages', 'chatMessages'].forEach(id => {
                    const container = document.getElementById(id);
                    if (!container) return;
                    const loader = document.createElement('div');
                    loader.id = `loader-${id}`;
                    loader.className = 'flex w-full mt-2 space-x-3';
                    loader.innerHTML = `
                        <div class="p-3 bg-slate-800/80 rounded-xl msg-ai flex items-center gap-2 border border-white/5">
                            <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce"></div>
                            <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
                            <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
                        </div>
                    `;
                    container.appendChild(loader);
                    container.scrollTop = container.scrollHeight;
                });
            },

            hideLoading: () => {
                ['loader-aiMainMessages', 'loader-chatMessages'].forEach(id => {
                    const el = document.getElementById(id);
                    if (el) el.remove();
                });
            }
        }
    };
})();
