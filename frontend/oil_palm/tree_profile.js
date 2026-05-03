document.addEventListener('DOMContentLoaded', async () => {
    const params = new URLSearchParams(window.location.search);
    let code = params.get('code');
    const barcode = params.get('barcode');
    const loading = document.getElementById('loading');
    const content = document.getElementById('profile-content');
    let currentTree = null;
    let currentSessionId = null;

    if (!code && barcode) {
        loading.textContent = 'Looking up barcode...';
        try {
            const res = await fetch(`/api/v1/trees/by-barcode/${encodeURIComponent(barcode)}`);
            const data = await res.json();
            if (data.status === 'ok' && data.tree?.tree_code) {
                code = data.tree.tree_code;
            }
        } catch (e) {
            loading.textContent = 'Barcode lookup failed: ' + e.message;
            return;
        }
    }

    if (!code) {
        loading.textContent = 'Error: No tree code or barcode provided. Use ?code=OP-XXXXXX or ?barcode=OP-XXXXXX';
        return;
    }

    document.getElementById('profile-title').textContent = `\u{1F334} Tree Profile: ${code}`;

    try {
        const res = await fetch(`/api/v1/trees/${code}`);
        if (!res.ok) {
            loading.textContent = `Error: Tree "${code}" not found (HTTP ${res.status})`;
            return;
        }
        const data = await res.json();
        const tree = data.tree;
        currentTree = tree;

        renderBasicInfo(tree);
        renderLocationInfo(tree);
        renderActions(tree, code);
        bindSessionActions(() => currentTree, () => currentSessionId, (id) => { 
            currentSessionId = id; 
            loadSessionImages(id);
        });
        loadBarcode(code);

        loading.style.display = 'none';
        content.style.display = 'block';
    } catch (e) {
        loading.textContent = 'Error loading tree: ' + e.message;
        return;
    }

    try {
        const tlRes = await fetch(`/api/v1/trees/${code}/timeline`);
        const tlData = await tlRes.json();
        renderTimeline(tlData.timeline || []);
    } catch (e) {
        document.getElementById('timeline-content').innerHTML =
            '<div class="timeline-empty">Failed to load timeline</div>';
    }
});

async function loadBarcode(code) {
    const el = document.getElementById('barcode-box');
    try {
        const res = await fetch(`/api/v1/trees/${code}/barcode`);
        const data = await res.json();
        if (data.status === 'ok') {
            el.textContent = `Barcode: ${data.barcode_value}`;
        } else {
            el.textContent = 'Barcode: unavailable';
        }
    } catch (e) {
        el.textContent = 'Barcode: failed to load';
    }
}

function renderBasicInfo(tree) {
    const statusClass = `badge-${tree.current_status}`;
    document.getElementById('basic-info').innerHTML = `
        <div class="info-row"><span class="info-label">Tree Code</span><span class="info-value">${tree.tree_code}</span></div>
        <div class="info-row"><span class="info-label">Species</span><span class="info-value">${tree.species}</span></div>
        <div class="info-row"><span class="info-label">Status</span><span class="info-value"><span class="badge ${statusClass}">${tree.current_status}</span></span></div>
        <div class="info-row"><span class="info-label">Barcode</span><span class="info-value">${tree.barcode_value || '-'}</span></div>
        <div class="info-row"><span class="info-label">Verified</span><span class="info-value">${tree.manual_verified ? '\u2705 Yes' : '\u274C No'}</span></div>
        <div class="info-row"><span class="info-label">Plantation</span><span class="info-value">${tree.plantation_name || '-'}</span></div>
        <div class="info-row"><span class="info-label">Created</span><span class="info-value">${formatDate(tree.created_at)}</span></div>
    `;
}

function renderLocationInfo(tree) {
    document.getElementById('location-info').innerHTML = `
        <div class="info-row"><span class="info-label">Coordinate X</span><span class="info-value">${tree.coordinate_x ?? '-'}</span></div>
        <div class="info-row"><span class="info-label">Coordinate Y</span><span class="info-value">${tree.coordinate_y ?? '-'}</span></div>
        <div class="info-row"><span class="info-label">Crown Center X</span><span class="info-value">${tree.crown_center_x ?? '-'}</span></div>
        <div class="info-row"><span class="info-label">Crown Center Y</span><span class="info-value">${tree.crown_center_y ?? '-'}</span></div>
        <div class="info-row"><span class="info-label">Source Ortho ID</span><span class="info-value">${tree.source_orthomosaic_id ?? '-'}</span></div>
        <div class="info-row"><span class="info-label">Block</span><span class="info-value">${tree.block_id || '-'}</span></div>
    `;
}

function renderTimeline(timeline) {
    const el = document.getElementById('timeline-content');
    if (timeline.length === 0) {
        el.innerHTML = '<div class="timeline-empty">\u{1F4ED} No history records yet. Timeline will be populated when future UAV missions match this tree.</div>';
        return;
    }
    el.innerHTML = timeline.map(t => `
        <div class="timeline-item">
            <strong>${t.mission_name}</strong>
            <span style="color:rgba(255,255,255,0.4); margin-left:8px;">${formatDate(t.mission_date || t.created_at)}</span>
            <div style="margin-top:4px; font-size:0.88rem; color:#9ca3af;">
                Detected: (${t.detected_x?.toFixed(2) ?? '-'}, ${t.detected_y?.toFixed(2) ?? '-'})
                &bull; Shift: ${t.center_shift?.toFixed(3) ?? '-'}
                &bull; Confidence: ${t.match_confidence?.toFixed(2) ?? '-'}
            </div>
        </div>
    `).join('');
}

function renderActions(tree, code) {
    const bar = document.getElementById('action-bar');
    const statuses = ['active', 'dead', 'removed', 'replanted'];
    statuses.forEach(s => {
        if (s === tree.current_status) return;
        const btn = document.createElement('button');
        btn.className = 'btn-status' + (s === 'dead' || s === 'removed' ? ' danger' : '');
        btn.textContent = `Mark as ${s}`;
        btn.addEventListener('click', async () => {
            if (!confirm(`Change status to "${s}"?`)) return;
            try {
                const res = await fetch(`/api/v1/trees/${code}/status`, {
                    method: 'PUT',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ status: s })
                });
                if (res.ok) {
                    window.location.reload();
                } else {
                    const err = await res.json();
                    alert('Failed: ' + (err.message || 'unknown error'));
                }
            } catch (e) {
                alert('Error: ' + e.message);
            }
        });
        bar.appendChild(btn);
    });
}

function bindSessionActions(getTree, getSessionId, setSessionId) {
    const startBtn = document.getElementById('btn-start-session');
    const uploadBtn = document.getElementById('btn-upload-session-image');
    const fileInput = document.getElementById('session-image');
    const roleInput = document.getElementById('image-role');
    const status = document.getElementById('session-status');
    const result = document.getElementById('session-result');

    fileInput.addEventListener('change', () => {
        uploadBtn.disabled = !getSessionId() || !fileInput.files.length;
    });

    startBtn.addEventListener('click', async () => {
        const tree = getTree();
        if (!tree || !tree.id) return;
        status.textContent = 'Creating observation session...';
        try {
            const res = await fetch(`/api/v1/trees/${tree.id}/sessions`, { method: 'POST' });
            const data = await res.json();
            if (data.status === 'ok') {
                setSessionId(data.session.id);
                status.textContent = `Active session: ${data.session.session_code}`;
                uploadBtn.disabled = !fileInput.files.length;
            } else {
                status.textContent = 'Session failed: ' + (data.message || 'unknown error');
            }
        } catch (e) {
            status.textContent = 'Session error: ' + e.message;
        }
    });

    uploadBtn.addEventListener('click', async () => {
        const sessionId = getSessionId();
        const file = fileInput.files[0];
        const role = roleInput.value;
        if (!sessionId || !file) return;
        const form = new FormData();
        form.append('image_role', role);
        form.append('file', file);
        status.textContent = 'Uploading session image...';
        try {
            // 我们同时在 URL 中带上 image_role 作为后端解析器的兜底
            const res = await fetch(`/api/v1/sessions/${sessionId}/images?image_role=${role}`, {
                method: 'POST',
                body: form
            });
            const data = await res.json();
            if (data.status === 'ok') {
                status.textContent = `Uploaded ${data.image.image_role} image`;
                fileInput.value = '';
                uploadBtn.disabled = true;
                // 刷新图片列表
                loadSessionImages(sessionId);
            } else {
                status.textContent = 'Upload failed: ' + (data.message || 'unknown error');
            }
        } catch (e) {
            status.textContent = 'Upload error: ' + e.message;
        }
    });
}

async function loadSessionImages(sessionId) {
    const el = document.getElementById('session-result');
    if (!sessionId) return;
    try {
        const res = await fetch(`/api/v1/sessions/${sessionId}/images`);
        const data = await res.json();
        if (data.status === 'ok' && data.images.length > 0) {
            el.innerHTML = `
                <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 10px;">
                    ${data.images.map(img => `
                        <div class="info-card" style="padding:8px; border:1px solid rgba(255,255,255,0.1);">
                            <img src="${fixImageUrl(img.image_url, img.upload_id)}" style="width:100%; border-radius:4px; aspect-ratio:1; object-fit:cover;">
                            <div style="font-size:0.75rem; margin-top:5px; color:#60a5fa; font-weight:700; text-transform:uppercase;">${img.image_role}</div>
                            <div style="font-size:0.7rem; color:rgba(255,255,255,0.5);">${formatDate(img.created_at)}</div>
                        </div>
                    `).join('')}
                </div>
                <div style="margin-top:15px; border-top:1px solid rgba(255,255,255,0.1); padding-top:10px;">
                    <h4 style="font-size:0.8rem; margin-bottom:5px; color:rgba(255,255,255,0.6);">Latest Analysis Detail</h4>
                    <pre style="font-size:0.75rem; color:#94a3b8; overflow-x:auto;">${JSON.stringify(data.images[data.images.length-1].mock_analysis, null, 2)}</pre>
                </div>
            `;
        }
    } catch (e) {
        console.error('Failed to load session images', e);
    }
}

function fixImageUrl(url, uploadId) {
    if (!url) return '';
    if (url.includes('api/v1/image/file')) return url;
    if (uploadId) return `/api/v1/image/file?upload_id=${uploadId}`;
    // 处理可能的物理路径
    const parts = url.split(/[\\/]/);
    const filename = parts[parts.length - 1];
    const idMatch = filename.match(/^(.+)\.\w+$/);
    const id = idMatch ? idMatch[1] : filename;
    return `/api/v1/image/file?upload_id=${id}`;
}

function formatDate(iso) {
    if (!iso) return '-';
    try {
        return new Date(iso).toLocaleDateString('en-US', {
            year: 'numeric', month: 'short', day: 'numeric',
            hour: '2-digit', minute: '2-digit'
        });
    } catch { return iso; }
}
