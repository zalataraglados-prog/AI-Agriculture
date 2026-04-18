/**
 * API & Data Logic Module
 * Handles fetching, schema management, fault detection, and mock fallbacks.
 */

window.API = (() => {
    const GATEWAY_STALE_MS = 5 * 60 * 1000;
    let schemaBySensor = new Map();

    const fetchJson = async (url) => {
        const res = await fetch(url, { cache: 'no-store' });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    };

    const loadSchema = async () => {
        try {
            const payload = await fetchJson('/api/v1/sensor/schema');
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
        } catch (err) {
            console.warn("Schema loading failed:", err);
        }
    };

    const detectSensorFault = (record) => {
        const sensorId = record.sensor_id;
        const fields = record.fields || {};
        const reasons = [];
        
        const sensorSchema = schemaBySensor.get(sensorId);
        if (!sensorSchema) return { isFault: false, reasons };

        // Check required fields
        const required = Array.from(sensorSchema.fields.values()).filter(v => v.required).map(v => v.field);
        required.forEach(name => {
            if (fields[name] === undefined || fields[name] === null || `${fields[name]}`.trim() === '') {
                reasons.push(`缺少必填字段:${name}`);
            }
        });

        // Check thresholds
        sensorSchema.fields.forEach((spec, field) => {
            const value = fields[field];
            if (value === undefined || value === null || value === '') return;
            const num = Number(value);
            if (isNaN(num)) return;
            if (spec.threshold_low !== null && num < spec.threshold_low) reasons.push(`字段过低:${field}`);
            if (spec.threshold_high !== null && num > spec.threshold_high) reasons.push(`字段过高:${field}`);
        });

        return { isFault: reasons.length > 0, reasons };
    };

    const getMockSensors = () => {
        return [
            { sensor_id: 'soil_modbus_02', device_id: 'GATEWAY-01', fields: { ec: 1.2, moisture: 45, temp: 22 }, ts: new Date().toISOString() },
            { sensor_id: 'dht22', device_id: 'GATEWAY-01', fields: { humidity: 65, temperature: 24 }, ts: new Date().toISOString() },
            { sensor_id: 'mq7', device_id: 'GATEWAY-01', fields: { co_ppm: 12 }, ts: new Date().toISOString() },
            { sensor_id: 'light_v1', device_id: 'GATEWAY-01', fields: { lx: 1200 }, ts: new Date().toISOString() }
        ];
    };

    const getSectors = () => {
        return [
            { id: 'sector-01-a', name: 'Sector 01-A (Rice)', status: 'active' },
            { id: 'sector-01-b', name: 'Sector 01-B (Corn)', status: 'active' },
            { id: 'sector-02-a', name: 'Sector 02-A (Fruit)', status: 'standby' }
        ];
    };

    const fetchHistory = async (sectorId, hours = 24) => {
        // In a real scenario, this calls /api/v1/telemetry?device_id=...&hours=...
        // For now, we return high-fidelity mock history
        const points = [];
        const now = Date.now();
        const count = hours * 4; // 15min intervals
        for (let i = 0; i < count; i++) {
            const ts = new Date(now - (count - i) * 15 * 60 * 1000).toISOString();
            points.push({
                sensor_id: 'soil_modbus_02',
                fields: { ec: 1.0 + Math.random() * 0.5, moisture: 40 + Math.random() * 10 },
                ts
            });
            points.push({
                sensor_id: 'dht22',
                fields: { humidity: 60 + Math.random() * 10, temperature: 22 + Math.random() * 5 },
                ts
            });
        }
        return points;
    };

    return {
        GATEWAY_STALE_MS,
        getSchema: () => schemaBySensor,
        fetchJson,
        loadSchema,
        detectSensorFault,
        getMockSensors,
        getSectors,
        fetchHistory,
        apiUrl: (base, queryObj) => {
            const u = new URL(base, window.location.origin);
            Object.entries(queryObj).forEach(([k, v]) => {
                if (v !== undefined && v !== null && `${v}`.trim() !== '') u.searchParams.set(k, v);
            });
            return u.toString();
        }
    };
})();
