document.addEventListener('DOMContentLoaded', async () => {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    const loading = document.getElementById('loading');
    const content = document.getElementById('profile-content');

    if (!code) {
        loading.textContent = 'Error: No tree code provided. Use ?code=OP-XXXXXX';
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

        renderBasicInfo(tree);
        renderLocationInfo(tree);
        renderActions(tree, code);

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

function formatDate(iso) {
    if (!iso) return '-';
    try {
        return new Date(iso).toLocaleDateString('en-US', {
            year: 'numeric', month: 'short', day: 'numeric',
            hour: '2-digit', minute: '2-digit'
        });
    } catch { return iso; }
}
