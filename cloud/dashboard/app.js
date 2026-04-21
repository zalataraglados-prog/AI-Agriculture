const params = new URLSearchParams(location.search);
const deviceId = (params.get('device_id') || localStorage.getItem('device_id') || '').trim();
if (deviceId) localStorage.setItem('device_id', deviceId);
document.getElementById('ctxDevice').textContent = `设备: ${deviceId || 'all'}`;

const GATEWAY_STALE_MS = 5 * 60 * 1000;
const DISEASE_THRESHOLD = 0.5;

let fertilityChart;
let faultTrendChart;
let schemaBySensor = new Map();

function formatTime(ts) {
  if (!ts) return '-';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  return `${mm}-${dd} ${hh}:${mi}:${ss}`;
}

function asNumber(v) {
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function showNotice(message) {
  const el = document.getElementById('schemaNotice');
  el.textContent = message;
  el.classList.remove('hidden');
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
  if (!sensor) return null;
  return sensor.fields.get(field) || null;
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
    if (schemaBySensor.size === 0) {
      showNotice('语义接口返回为空，部分故障判定将降级。');
    }
  } catch (err) {
    showNotice(`语义接口不可用：${err.message}。仅展示真实值，故障判定降级。`);
  }
}

function detectSensorFault(record) {
  const sensorId = record.sensor_id;
  const fields = record.fields || {};
  const reasons = [];
  const required = getRequiredFields(sensorId);
  required.forEach(name => {
    const v = fields[name];
    if (v === undefined || v === null || `${v}`.trim?.() === '') {
      reasons.push(`缺少必填字段:${name}`);
    }
  });

  const sensorSchema = schemaBySensor.get(sensorId);
  if (sensorSchema) {
    sensorSchema.fields.forEach((spec, field) => {
      const value = fields[field];
      if (value === undefined || value === null || value === '') return;
      if (!['u8', 'u16', 'u32', 'i32', 'f32', 'f64'].includes(spec.data_type)) return;
      const num = asNumber(value);
      if (num === null) {
        reasons.push(`字段非数值:${field}`);
        return;
      }
      if (spec.threshold_low !== null && num < spec.threshold_low) reasons.push(`字段过低:${field}`);
      if (spec.threshold_high !== null && num > spec.threshold_high) reasons.push(`字段过高:${field}`);
    });
  }

  return {
    isFault: reasons.length > 0,
    reasons
  };
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
  latestMap.forEach((item, deviceId) => {
    if (!Number.isFinite(item.tsMs)) {
      out.add(deviceId);
      return;
    }
    if (nowMs - item.tsMs > GATEWAY_STALE_MS) out.add(deviceId);
  });
  return out;
}

function sensorFaultDevices(latestMap) {
  const out = new Set();
  const reasonByDevice = new Map();
  latestMap.forEach((item, deviceId) => {
    const fault = detectSensorFault(item.row);
    if (fault.isFault) {
      out.add(deviceId);
      reasonByDevice.set(deviceId, fault.reasons.join('；'));
    }
  });
  return { devices: out, reasonByDevice };
}

function fertilitySeries(telemetry) {
  const rows = telemetry
    .filter(r => r.sensor_id === 'soil_modbus_02')
    .map(r => ({ tsMs: Date.parse(r.ts || ''), ts: r.ts, ec: asNumber(r?.fields?.ec) }))
    .filter(x => Number.isFinite(x.tsMs) && x.ec !== null)
    .sort((a, b) => a.tsMs - b.tsMs);
  return rows;
}

function faultTrendSeries(telemetry, nowMs) {
  const sensorBuckets = new Map();
  const gatewayBuckets = new Map();

  const bucketKey = (tsMs) => {
    const d = new Date(tsMs);
    d.setSeconds(0, 0);
    return d.getTime();
  };

  telemetry.forEach(row => {
    const tsMs = Date.parse(row.ts || '');
    if (!Number.isFinite(tsMs)) return;
    const key = bucketKey(tsMs);
    const fault = detectSensorFault(row);
    if (fault.isFault) sensorBuckets.set(key, (sensorBuckets.get(key) || 0) + 1);
  });

  const byDevice = new Map();
  telemetry.forEach(row => {
    const id = row.device_id || 'unknown';
    if (!byDevice.has(id)) byDevice.set(id, []);
    byDevice.get(id).push(Date.parse(row.ts || ''));
  });
  byDevice.forEach(list => {
    const points = list.filter(Number.isFinite).sort((a, b) => a - b);
    for (let i = 1; i < points.length; i += 1) {
      if (points[i] - points[i - 1] > GATEWAY_STALE_MS) {
        const key = bucketKey(points[i]);
        gatewayBuckets.set(key, (gatewayBuckets.get(key) || 0) + 1);
      }
    }
    if (points.length && nowMs - points[points.length - 1] > GATEWAY_STALE_MS) {
      const key = bucketKey(nowMs);
      gatewayBuckets.set(key, (gatewayBuckets.get(key) || 0) + 1);
    }
  });

  const keys = Array.from(new Set([...sensorBuckets.keys(), ...gatewayBuckets.keys()])).sort((a, b) => a - b);
  return {
    labels: keys.map(k => formatTime(new Date(k).toISOString())),
    sensorFault: keys.map(k => sensorBuckets.get(k) || 0),
    gatewayFault: keys.map(k => gatewayBuckets.get(k) || 0)
  };
}

function fmtRate(value) {
  const n = asNumber(value);
  if (n === null) return null;
  if (n >= 0 && n <= 1) return n;
  if (n > 1 && n <= 100) return n / 100;
  return null;
}

async function loadData() {
  const telemetryUrl = apiUrl('/api/v1/telemetry', { device_id: deviceId, limit: 300 });
  const imageUrl = apiUrl('/api/v1/image/uploads', { device_id: deviceId, limit: 50 });
  const [telemetry, imageUploads] = await Promise.all([
    fetchJson(telemetryUrl).catch(() => []),
    fetchJson(imageUrl).catch(() => [])
  ]);

  const nowMs = Date.now();
  const latestMap = buildDeviceLatest(telemetry);
  const gatewaySet = gatewayFaultDevices(latestMap, nowMs);
  const sensorFault = sensorFaultDevices(latestMap);
  const faultDeviceSet = new Set([...gatewaySet, ...sensorFault.devices]);

  const soilRows = fertilitySeries(telemetry);
  const avgEc = soilRows.length ? (soilRows.reduce((sum, r) => sum + r.ec, 0) / soilRows.length) : null;

  const diseaseRates = imageUploads
    .map(r => fmtRate(r.disease_rate))
    .filter(v => v !== null);
  const avgDiseaseRate = diseaseRates.length
    ? diseaseRates.reduce((a, b) => a + b, 0) / diseaseRates.length
    : null;

  document.getElementById('valDeviceCount').textContent = latestMap.size || '-';
  document.getElementById('valAvgEc').textContent = avgEc === null ? '-' : `${avgEc.toFixed(1)} μS/cm`;
  document.getElementById('valFaultDevices').textContent = faultDeviceSet.size || '0';
  document.getElementById('valAvgDiseaseRate').textContent = avgDiseaseRate === null ? '-' : `${(avgDiseaseRate * 100).toFixed(1)}%`;

  fertilityChart.data.labels = soilRows.map(r => formatTime(r.ts));
  fertilityChart.data.datasets[0].data = soilRows.map(r => r.ec);
  fertilityChart.update();

  const faultTrend = faultTrendSeries(telemetry, nowMs);
  faultTrendChart.data.labels = faultTrend.labels;
  faultTrendChart.data.datasets[0].data = faultTrend.sensorFault;
  faultTrendChart.data.datasets[1].data = faultTrend.gatewayFault;
  faultTrendChart.update();

  const telemetryBody = document.getElementById('telemetryBody');
  const telemetryRows = [...telemetry].sort((a, b) => Date.parse(b.ts || '') - Date.parse(a.ts || '')).slice(0, 10);
  if (!telemetryRows.length) {
    telemetryBody.innerHTML = '<tr><td colspan="5" class="py-4 text-center text-gray-500">暂无数据</td></tr>';
  } else {
    telemetryBody.innerHTML = telemetryRows.map(row => {
      const device = row.device_id || '-';
      const ec = asNumber(row?.fields?.ec);
      const stale = gatewaySet.has(device);
      const sensorResult = detectSensorFault(row);
      let status = '正常';
      let statusCls = 'text-green-700 bg-green-50';
      let detail = '数据正常';
      if (stale) {
        status = '网关故障';
        statusCls = 'text-red-700 bg-red-50';
        detail = '最近5分钟无上报';
      } else if (sensorResult.isFault) {
        status = '传感器故障';
        statusCls = 'text-yellow-700 bg-yellow-50';
        detail = sensorResult.reasons.join('；');
      }
      return `<tr class="border-b">
        <td class="py-3 px-2">${formatTime(row.ts)}</td>
        <td class="py-3 px-2">${device}<span class="ml-1 text-xs text-gray-400">${row.sensor_id || ''}</span></td>
        <td class="py-3 px-2">${ec === null ? '-' : `${ec.toFixed(1)} μS/cm`}</td>
        <td class="py-3 px-2"><span class="text-xs px-2 py-1 rounded-full ${statusCls}">${status}</span></td>
        <td class="py-3 px-2 text-xs text-gray-600">${detail}</td>
      </tr>`;
    }).join('');
  }

  const imageBody = document.getElementById('imageBody');
  const imageRows = [...imageUploads].sort((a, b) => Date.parse(b.captured_at || '') - Date.parse(a.captured_at || '')).slice(0, 10);
  if (!imageRows.length) {
    imageBody.innerHTML = '<tr><td colspan="5" class="py-4 text-center text-gray-500">暂无数据</td></tr>';
  } else {
    imageBody.innerHTML = imageRows.map(row => {
      const isLinkFault = row.upload_status === 'failed';
      const diseaseRate = fmtRate(row.disease_rate);
      const diseased = typeof row.is_diseased === 'boolean'
        ? row.is_diseased
        : (diseaseRate !== null ? diseaseRate >= DISEASE_THRESHOLD : null);
      const faultText = isLinkFault ? '是' : '否';
      const faultCls = isLinkFault ? 'text-red-700 bg-red-50' : 'text-green-700 bg-green-50';
      const diseaseText = diseaseRate === null
        ? '-'
        : `${(diseaseRate * 100).toFixed(1)}%${diseased === null ? '' : (diseased ? ' (疑似患病)' : ' (疑似健康)')}`;
      return `<tr class="border-b">
        <td class="py-3 px-2">${formatTime(row.captured_at)}</td>
        <td class="py-3 px-2">${row.device_id || '-'}</td>
        <td class="py-3 px-2"><span class="text-xs px-2 py-1 rounded-full ${faultCls}">${faultText}</span></td>
        <td class="py-3 px-2">${diseaseText}</td>
        <td class="py-3 px-2">${row.predicted_class || '-'}</td>
      </tr>`;
    }).join('');
  }
}

window.onload = async () => {
  fertilityChart = new Chart(document.getElementById('fertilityChart').getContext('2d'), {
    type: 'line',
    data: {
      labels: [],
      datasets: [{
        label: 'EC(μS/cm)',
        data: [],
        borderColor: '#16a34a',
        backgroundColor: 'rgba(22,163,74,0.1)',
        tension: 0.25,
        fill: true,
        pointRadius: 2
      }]
    },
    options: { responsive: true, maintainAspectRatio: false }
  });

  faultTrendChart = new Chart(document.getElementById('faultTrendChart').getContext('2d'), {
    type: 'line',
    data: {
      labels: [],
      datasets: [
        {
          label: '传感器故障数',
          data: [],
          borderColor: '#f59e0b',
          backgroundColor: 'rgba(245,158,11,0.15)',
          tension: 0.2,
          fill: true,
          pointRadius: 2
        },
        {
          label: '网关故障数',
          data: [],
          borderColor: '#ef4444',
          backgroundColor: 'rgba(239,68,68,0.15)',
          tension: 0.2,
          fill: true,
          pointRadius: 2
        }
      ]
    },
    options: { responsive: true, maintainAspectRatio: false }
  });

  await loadSchema();
  await loadData();
  setInterval(loadData, 15000);
};
