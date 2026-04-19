/**
 * API & data state module (real-data only).
 */

window.API = (() => {
    const GATEWAY_STALE_MS = 5 * 60 * 1000;
    const DEFAULT_LIMIT = 300;
    const schemaBySensor = new Map();
    let telemetryRecords = [];

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

    const loadSchema = async () => {
        schemaBySensor.clear();
        try {
            const payload = await fetchJson('/api/v1/sensor/schema');
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
        } catch (err) {
            console.warn('Schema loading failed:', err);
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
            const numericTypes = ['number', 'float', 'f32', 'f64', 'u8', 'u16', 'u32', 'i32'];
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

    const fetchHistory = async (deviceId, hours = 24, limit = 1000, explicitStart = null, explicitEnd = null) => {
        const end = explicitEnd ? new Date(explicitEnd) : new Date();
        const start = explicitStart ? new Date(explicitStart) : new Date(end.getTime() - hours * 3600 * 1000);
        
        const url = apiUrl('/api/v1/telemetry', {
            device_id: deviceId,
            start_time: start.toISOString(),
            end_time: end.toISOString(),
            limit: Math.max(DEFAULT_LIMIT, Math.min(limit, 1000)),
        });
        const rows = await fetchJson(url);
        return normalizeTelemetryRows(rows).sort((a, b) => new Date(a.ts).getTime() - new Date(b.ts).getTime());
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
    };
})();
