/**
 * API & data state module (real-data only).
 */

window.API = (() => {
    const runtime = window.RUNTIME_CONFIG || {};
    const telemetryCfg = runtime.telemetry || {};
    const uploadCfg = runtime.imageUpload || {};
    const GATEWAY_STALE_MS = Number(telemetryCfg.gatewayStaleMs) || 5 * 60 * 1000;
    const DEFAULT_LIMIT = Number(telemetryCfg.defaultLimit) || 300;
    const HISTORY_MAX_LIMIT = Number(telemetryCfg.historyMaxLimit) || 1000;
    const DEFAULT_UPLOAD_RETRIES = Number(uploadCfg.retries);
    const DEFAULT_UPLOAD_TIMEOUT_MS = Number(uploadCfg.timeoutMs);
    const schemaBySensor = new Map();
    let schemaSource = 'remote';
    let telemetryRecords = [];
    const FALLBACK_SCHEMA = {
        sensors: [
            {
                sensor_id: 'soil_modbus_02',
                trend_metric: 'ec',
                category_metric: 'slave_id',
                fields: [
                    { field: 'vwc', label: 'Soil Moisture', unit: '%', data_type: 'f32', required: true, threshold_low: 20, threshold_high: 70 },
                    { field: 'temp_c', label: 'Temperature', unit: 'C', data_type: 'f32', required: true, threshold_low: 0, threshold_high: 45 },
                    { field: 'ec', label: 'Soil Fertility', unit: 'uS/cm', data_type: 'f32', required: true, threshold_low: 0, threshold_high: 5000 },
                ],
            },
            {
                sensor_id: 'dht22',
                trend_metric: 'temp_c',
                category_metric: null,
                fields: [
                    { field: 'temp_c', label: 'Temperature', unit: 'C', data_type: 'f32', required: true, threshold_low: 0, threshold_high: 50 },
                    { field: 'hum', label: 'Humidity', unit: '%', data_type: 'f32', required: true, threshold_low: 20, threshold_high: 95 },
                ],
            },
        ],
    };

    const fetchJson = async (url) => {
        const res = await fetch(url, { cache: 'no-store' });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    };

    const apiUrl = (base, queryObj = {}) => {
        const u = new URL(base, window.location.origin);
        Object.entries(queryObj).forEach(([k, v]) => {
            if (v === undefined || v === null) return;
            const text = `${v}`.trim();
            if (!text) return;
            u.searchParams.set(k, text);
        });
        return u.toString();
    };

    const normalizeTelemetryRows = (rows) => {
        if (!Array.isArray(rows)) return [];
        return rows
            .map((row) => {
                const fields = row?.fields && typeof row.fields === 'object' ? row.fields : {};
                return {
                    ts: row?.ts || row?.received_at || null,
                    device_id: row?.device_id || '',
                    sensor_id: row?.sensor_id || '',
                    fields,
                };
            })
            .filter((row) => row.sensor_id);
    };

    const setTelemetry = (rows) => {
        telemetryRecords = normalizeTelemetryRows(rows).sort((a, b) => {
            const ta = new Date(a.ts || 0).getTime();
            const tb = new Date(b.ts || 0).getTime();
            return tb - ta;
        });
    };

    const getTelemetry = () => telemetryRecords.slice();

    const getLatestBySensor = (sensorId) => telemetryRecords.find((row) => row.sensor_id === sensorId) || null;

    const loadSchemaFromPayload = (payload) => {
        schemaBySensor.clear();
        const sensors = Array.isArray(payload?.sensors) ? payload.sensors : [];
        sensors.forEach((sensor) => {
            if (!sensor?.sensor_id) return;
            const fields = new Map();
            (sensor.fields || []).forEach((field) => {
                if (!field?.field) return;
                fields.set(field.field, {
                    field: field.field,
                    label: field.label || field.field,
                    unit: field.unit || '',
                    data_type: field.data_type || 'string',
                    required: !!field.required,
                    threshold_low: typeof field.threshold_low === 'number' ? field.threshold_low : null,
                    threshold_high: typeof field.threshold_high === 'number' ? field.threshold_high : null,
                });
            });
            schemaBySensor.set(sensor.sensor_id, {
                trendMetric: sensor.trend_metric || null,
                categoryMetric: sensor.category_metric || null,
                fields,
            });
        });
    };

    const loadSchema = async () => {
        schemaBySensor.clear();
        schemaSource = 'remote';
        try {
            const payload = await fetchJson('/api/v1/sensor/schema');
            loadSchemaFromPayload(payload);
            if (!schemaBySensor.size) {
                loadSchemaFromPayload(FALLBACK_SCHEMA);
                schemaSource = 'fallback';
            }
        } catch (err) {
            console.warn('Schema loading failed:', err);
            loadSchemaFromPayload(FALLBACK_SCHEMA);
            schemaSource = 'fallback';
        }
    };

    const detectSensorFault = (record) => {
        const reasons = [];
        if (!record) return { isFault: true, reasons: ['NO_TELEMETRY'] };

        const tsMs = new Date(record.ts || 0).getTime();
        if (!Number.isFinite(tsMs) || Date.now() - tsMs > GATEWAY_STALE_MS) {
            reasons.push('STALE_TELEMETRY');
        }

        const sensorSchema = schemaBySensor.get(record.sensor_id);
        if (!sensorSchema) return { isFault: reasons.length > 0, reasons };

        const fields = record.fields || {};
        const required = Array.from(sensorSchema.fields.values()).filter((v) => v.required).map((v) => v.field);
        required.forEach((name) => {
            const value = fields[name];
            if (value === undefined || value === null || `${value}`.trim() === '') {
                reasons.push(`MISSING:${name}`);
            }
        });

        sensorSchema.fields.forEach((spec, field) => {
            const value = fields[field];
            if (value === undefined || value === null || value === '') return;
            const dataType = `${spec.data_type || ''}`.toLowerCase();
            const numericTypes = ['number', 'float', 'f32', 'f64', 'u8', 'u16', 'u32', 'u64', 'i32', 'i64'];
            if (!numericTypes.includes(dataType)) return;
            const num = Number(value);
            if (!Number.isFinite(num)) {
                reasons.push(`NON_NUMERIC:${field}`);
                return;
            }
            if (spec.threshold_low !== null && num < spec.threshold_low) reasons.push(`LOW:${field}`);
            if (spec.threshold_high !== null && num > spec.threshold_high) reasons.push(`HIGH:${field}`);
        });

        return { isFault: reasons.length > 0, reasons };
    };

    const getSchemaField = (sensorId, fieldName) => schemaBySensor.get(sensorId)?.fields?.get(fieldName) || null;

    const formatNumeric = (value, unit = '') => {
        const num = Number(value);
        if (!Number.isFinite(num)) return '-';
        const abs = Math.abs(num);
        const digits = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
        const suffix = unit ? ` ${unit}` : '';
        return `${num.toFixed(digits)}${suffix}`;
    };

    const parseInputDate = (value) => {
        if (!value) return null;
        const text = `${value}`.trim();
        if (!text) return null;
        const dt = new Date(text);
        if (!Number.isFinite(dt.getTime())) return null;
        return dt;
    };

    const fetchHistory = async (deviceIds, hours = 24, limit = 1000, explicitStart = null, explicitEnd = null) => {
        const ids = Array.isArray(deviceIds) ? deviceIds : [deviceIds];
        if (!ids.length) return [];
        const parsedEnd = parseInputDate(explicitEnd);
        const parsedStart = parseInputDate(explicitStart);
        const end = parsedEnd || new Date();
        const start = parsedStart || new Date(end.getTime() - hours * 3600 * 1000);
        // datetime-local values are usually minute precision, while backend uses [start, end).
        // Expand end by one minute to keep user-selected final minute included.
        const endExclusive = explicitEnd ? new Date(end.getTime() + 60 * 1000) : end;

        // Retry a single fetch up to maxRetries times with a small delay
        const fetchWithRetry = async (url, maxRetries = 2, delayMs = 600) => {
            for (let attempt = 0; attempt <= maxRetries; attempt++) {
                try {
                    const result = await fetchJson(url);
                    return Array.isArray(result) ? result : [];
                } catch (err) {
                    if (attempt === maxRetries) {
                        console.warn(`[API] fetchHistory: gave up after ${maxRetries + 1} attempts for ${url}`, err);
                        return [];
                    }
                    await new Promise(r => setTimeout(r, delayMs * (attempt + 1)));
                }
            }
            return [];
        };

        const fetchPromises = ids.map(id => {
            const url = apiUrl('/api/v1/telemetry', {
                device_id: id,
                start_time: start.toISOString(),
                end_time: endExclusive.toISOString(),
                limit: Math.max(DEFAULT_LIMIT, Math.min(limit, HISTORY_MAX_LIMIT)),
            });
            return fetchWithRetry(url);
        });

        const results = await Promise.all(fetchPromises);
        let combinedRows = [];
        results.forEach(rows => {
            if (Array.isArray(rows)) {
                combinedRows = combinedRows.concat(rows);
            }
        });

        // Deduplicate based on exact timestamp and sensor to prevent render glitches
        const uniqueRows = new Map();
        combinedRows.forEach(row => {
            const key = `${row.device_id || ''}_${row.sensor_id || ''}_${row.ts || ''}`;
            uniqueRows.set(key, row);
        });

        const merged = Array.from(uniqueRows.values());
        return normalizeTelemetryRows(merged).sort((a, b) => new Date(a.ts).getTime() - new Date(b.ts).getTime());
    };


    const fetchDevices = async () => {
        let devices = [];
        try {
            const data = await fetchJson(apiUrl('/api/v1/devices'));
            if (Array.isArray(data?.devices) && data.devices.length > 0) {
                devices = data.devices;
            }
        } catch (e) {
            console.warn('[API] fetchDevices failed:', e);
        }
        const cropTypes = [...new Set(devices.map(d => d.crop_type).filter(Boolean))];
        const locations = [...new Set(devices.map(d => d.location).filter(Boolean))];
        return { devices, cropTypes, locations };
    };

    const maybeConvertForUpload = async (file) => {
        if (!file) throw new Error('No image file selected');
        const type = `${file.type || ''}`.toLowerCase();
        const heicLike = type.includes('heic') || type.includes('heif') || /\.(heic|heif)$/i.test(file.name || '');
        if (!heicLike) {
            return file;
        }
        if (typeof createImageBitmap !== 'function') {
            throw new Error('HEIC conversion is not supported on this browser. Please upload jpg/png.');
        }
        try {
            const imageBitmap = await createImageBitmap(file);
            const canvas = document.createElement('canvas');
            canvas.width = imageBitmap.width;
            canvas.height = imageBitmap.height;
            const ctx = canvas.getContext('2d');
            if (!ctx) throw new Error('Canvas context unavailable');
            ctx.drawImage(imageBitmap, 0, 0);
            imageBitmap.close();
            const jpegBlob = await new Promise((resolve, reject) => {
                canvas.toBlob(
                    (blob) => (blob ? resolve(blob) : reject(new Error('HEIC to JPEG conversion failed'))),
                    'image/jpeg',
                    0.92,
                );
            });
            const baseName = (file.name || 'upload').replace(/\.(heic|heif)$/i, '');
            return new File([jpegBlob], `${baseName}.jpg`, { type: 'image/jpeg' });
        } catch (err) {
            throw new Error(`HEIC conversion failed: ${err.message || err}`);
        }
    };

    const buildUploadQuery = (tag) =>
        apiUrl('/api/v1/image/upload', {
            device_id: tag?.device_id || '',
            ts: tag?.ts || new Date().toISOString(),
            location: tag?.location || '',
            crop_type: tag?.crop_type || '',
            farm_note: tag?.farm_note || '',
        });

    const uploadImageOnce = ({ file, tag, timeoutMs, onProgress }) =>
        new Promise((resolve, reject) => {
            const xhr = new XMLHttpRequest();
            xhr.open('POST', buildUploadQuery(tag), true);
            xhr.timeout = timeoutMs;
            xhr.onload = () => {
                const payloadText = xhr.responseText || '{}';
                let payload = {};
                try {
                    payload = JSON.parse(payloadText);
                } catch (err) {
                    reject(new Error(`Upload response is not JSON: ${payloadText.slice(0, 120)}`));
                    return;
                }
                if (xhr.status >= 200 && xhr.status < 300 && payload?.status === 'success') {
                    resolve(payload);
                    return;
                }
                const message = payload?.message || `HTTP ${xhr.status}`;
                reject(new Error(message));
            };
            xhr.onerror = () => reject(new Error('Network error while uploading image'));
            xhr.ontimeout = () => reject(new Error(`Upload timeout (${timeoutMs} ms)`));
            xhr.upload.onprogress = (evt) => {
                if (!evt.lengthComputable || typeof onProgress !== 'function') return;
                onProgress(Math.round((evt.loaded / evt.total) * 100));
            };

            const form = new FormData();
            form.append('file', file, file.name || 'upload.jpg');
            xhr.send(form);
        });

    const uploadImage = async ({
        file,
        tag,
        retries = Number.isFinite(DEFAULT_UPLOAD_RETRIES) ? DEFAULT_UPLOAD_RETRIES : 2,
        timeoutMs = Number.isFinite(DEFAULT_UPLOAD_TIMEOUT_MS) ? DEFAULT_UPLOAD_TIMEOUT_MS : 45000,
        onProgress,
    }) => {
        const deviceId = `${tag?.device_id || ''}`.trim();
        if (!deviceId) throw new Error('device_id is required for upload');
        const preparedFile = await maybeConvertForUpload(file);
        let lastErr = null;
        for (let attempt = 0; attempt <= retries; attempt += 1) {
            try {
                if (typeof onProgress === 'function') onProgress(0);
                const result = await uploadImageOnce({
                    file: preparedFile,
                    tag: { ...tag, device_id: deviceId, ts: tag?.ts || new Date().toISOString() },
                    timeoutMs,
                    onProgress,
                });
                if (typeof onProgress === 'function') onProgress(100);
                return result;
            } catch (err) {
                lastErr = err;
                if (attempt >= retries) break;
                await new Promise((resolve) => setTimeout(resolve, 800 * (attempt + 1)));
            }
        }
        throw lastErr || new Error('Image upload failed');
    };



    return {
        GATEWAY_STALE_MS,
        fetchJson,
        apiUrl,
        loadSchema,
        getSchema: () => schemaBySensor,
        getSchemaField,
        setTelemetry,
        getTelemetry,
        getLatestBySensor,
        detectSensorFault,
        formatNumeric,
        fetchHistory,
        fetchDevices,
        uploadImage,
        isSchemaFallback: () => schemaSource === 'fallback',
        getSchemaSource: () => schemaSource,
    };
})();
