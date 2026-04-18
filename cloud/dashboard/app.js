const params = new URLSearchParams(location.search);
const deviceId = (params.get('device_id') || localStorage.getItem('device_id') || '').trim();
if (deviceId) localStorage.setItem('device_id', deviceId);
document.getElementById('ctxDevice').textContent = `设备: ${deviceId || 'all'}`;

let humidityChart;
let typeChart;

function tsLabel(ts) {
  if (!ts) return '-';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
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

function statusBadge(vwc) {
  const value = Number(vwc);
  if (!Number.isFinite(value)) return { txt: '未知', cls: 'text-gray-600 bg-gray-50' };
  if (value >= 70) return { txt: '偏高', cls: 'text-yellow-700 bg-yellow-50' };
  if (value <= 20) return { txt: '偏低', cls: 'text-red-700 bg-red-50' };
  return { txt: '正常', cls: 'text-green-700 bg-green-50' };
}

function asNumber(v) {
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

async function loadData() {
  const telemetryUrl = apiUrl('/api/v1/telemetry', { device_id: deviceId, limit: 100 });
  const imagesUrl = apiUrl('/api/v1/image/uploads', { device_id: deviceId, limit: 20 });
  const [telemetry, imageUploads] = await Promise.all([
    fetchJson(telemetryUrl).catch(() => []),
    fetchJson(imagesUrl).catch(() => [])
  ]);

  const devices = new Set();
  const vwcList = [];
  const tempList = [];
  telemetry.forEach(r => {
    if (r.device_id) devices.add(r.device_id);
    const vwc = asNumber(r?.fields?.vwc);
    const temp = asNumber(r?.fields?.temp_c);
    if (vwc !== null) vwcList.push(vwc);
    if (temp !== null) tempList.push(temp);
  });

  const avg = list => list.length ? (list.reduce((a, b) => a + b, 0) / list.length) : null;
  const avgVwc = avg(vwcList);
  const avgTemp = avg(tempList);
  const inferred = imageUploads.filter(x => x.upload_status === 'inferred').length;
  const inferRate = imageUploads.length ? `${Math.round((inferred / imageUploads.length) * 100)}%` : '-';

  document.getElementById('valDeviceCount').textContent = devices.size || '-';
  document.getElementById('valAvgVwc').textContent = avgVwc === null ? '-' : `${avgVwc.toFixed(1)}%`;
  document.getElementById('valAvgTemp').textContent = avgTemp === null ? '-' : `${avgTemp.toFixed(1)}℃`;
  document.getElementById('valInferRate').textContent = inferRate;

  const trend = telemetry.slice(0, 7).reverse();
  humidityChart.data.labels = trend.map(x => tsLabel(x.ts));
  humidityChart.data.datasets[0].data = trend.map(x => asNumber(x?.fields?.vwc)).map(x => x ?? null);
  humidityChart.update();

  const cropMap = {};
  imageUploads.forEach(item => {
    const k = item.crop_type && item.crop_type.trim() ? item.crop_type.trim() : 'unknown';
    cropMap[k] = (cropMap[k] || 0) + 1;
  });
  const typeLabels = Object.keys(cropMap);
  typeChart.data.labels = typeLabels.length ? typeLabels : ['unknown'];
  typeChart.data.datasets[0].data = typeLabels.length ? typeLabels.map(k => cropMap[k]) : [1];
  typeChart.update();

  const telemetryBody = document.getElementById('telemetryBody');
  if (!telemetry.length) {
    telemetryBody.innerHTML = '<tr><td colspan="5" class="py-4 text-center text-gray-500">暂无数据</td></tr>';
  } else {
    telemetryBody.innerHTML = telemetry.slice(0, 10).map(r => {
      const vwc = asNumber(r?.fields?.vwc);
      const temp = asNumber(r?.fields?.temp_c);
      const badge = statusBadge(vwc);
      return `<tr class="border-b">
        <td class="py-3 px-2">${tsLabel(r.ts)}</td>
        <td class="py-3 px-2">${r.device_id || '-'}</td>
        <td class="py-3 px-2">${vwc === null ? '-' : `${vwc.toFixed(1)}%`}</td>
        <td class="py-3 px-2">${temp === null ? '-' : `${temp.toFixed(1)}℃`}</td>
        <td class="py-3 px-2"><span class="text-xs px-2 py-1 rounded-full ${badge.cls}">${badge.txt}</span></td>
      </tr>`;
    }).join('');
  }

  const imageBody = document.getElementById('imageBody');
  if (!imageUploads.length) {
    imageBody.innerHTML = '<tr><td colspan="5" class="py-4 text-center text-gray-500">暂无数据</td></tr>';
  } else {
    imageBody.innerHTML = imageUploads.slice(0, 10).map(r => {
      const statusClass = r.upload_status === 'inferred' ? 'text-green-700 bg-green-50' : (r.upload_status === 'failed' ? 'text-red-700 bg-red-50' : 'text-blue-700 bg-blue-50');
      const confidence = (typeof r.confidence === 'number') ? `${(r.confidence * 100).toFixed(1)}%` : '-';
      return `<tr class="border-b">
        <td class="py-3 px-2">${tsLabel(r.captured_at)}</td>
        <td class="py-3 px-2">${r.device_id || '-'}</td>
        <td class="py-3 px-2"><span class="text-xs px-2 py-1 rounded-full ${statusClass}">${r.upload_status || '-'}</span></td>
        <td class="py-3 px-2">${r.predicted_class || '-'}</td>
        <td class="py-3 px-2">${confidence}</td>
      </tr>`;
    }).join('');
  }
}

window.onload = () => {
  humidityChart = new Chart(document.getElementById('humidityChart').getContext('2d'), {
    type: 'line',
    data: { labels: [], datasets: [{ label: '土壤湿度(%)', data: [], borderColor: '#16a34a', backgroundColor: 'rgba(22,163,74,0.1)', tension: 0.3, fill: true }] },
    options: { responsive: true, maintainAspectRatio: false }
  });
  typeChart = new Chart(document.getElementById('typeChart').getContext('2d'), {
    type: 'doughnut',
    data: { labels: ['unknown'], datasets: [{ data: [1], backgroundColor: ['#16a34a','#0ea5e9','#f59e0b','#ef4444','#6366f1','#ef4444'] }] },
    options: { responsive: true, maintainAspectRatio: false }
  });
  loadData();
  setInterval(loadData, 15000);
};
