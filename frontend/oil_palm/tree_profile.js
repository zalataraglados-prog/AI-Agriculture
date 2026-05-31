document.addEventListener('DOMContentLoaded', async () => {
    const params = new URLSearchParams(window.location.search);
    let code = params.get('code');
    const barcode = params.get('barcode');
    const loading = document.getElementById('loading');
    const content = document.getElementById('profile-content');

    const state = {
        code: null,
        tree: null,
        timeline: [],
        assessment: null,
        assessmentMessage: null,
        sessionId: null,
        sessionCode: null,
        sessionCreating: false,
        sessionUploading: false,
        sessions: [],
        sessionImages: [],
        loadingMessage: null,
        barcodeMessage: null,
        sessionMessage: { key: 'no_active_session', params: {} },
        a0Message: { key: 'a0_review_hint', params: {} },
        a0: null
    };

    let updateSessionControls = () => {};

    function t(key, params = {}) {
        if (window.OP_I18N) return window.OP_I18N.t(key, params);
        return Object.entries(params).reduce((out, [name, value]) => out.replaceAll(`{${name}}`, value), key);
    }

    function escapeHtml(value) {
        return String(value ?? '')
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function unknownError(message) {
        return message || t('unknown_error');
    }

    function formatDate(iso) {
        if (!iso) return '-';
        try {
            const lang = window.OP_I18N?.getLanguage?.() || 'en';
            const locale = lang === 'zh' ? 'zh-CN' : (lang === 'ms' ? 'ms-MY' : 'en-US');
            return new Date(iso).toLocaleDateString(locale, {
                year: 'numeric',
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit'
            });
        } catch {
            return iso;
        }
    }

    function fixImageUrl(url, uploadId) {
        if (!url) return '';
        if (url.includes('api/v1/image/file')) return url;
        if (uploadId) return `/api/v1/image/file?upload_id=${encodeURIComponent(uploadId)}`;
        const parts = url.split(/[\\/]/);
        const filename = parts[parts.length - 1];
        const idMatch = filename.match(/^(.+)\.\w+$/);
        const id = idMatch ? idMatch[1] : filename;
        return `/api/v1/image/file?upload_id=${encodeURIComponent(id)}`;
    }

    function setLoading(key, params = {}) {
        state.loadingMessage = { key, params };
        loading.textContent = t(key, params);
    }

    function updateProfileTitle() {
        if (!state.code) return;
        document.getElementById('profile-title').textContent = t('profile_title_with_code', { code: state.code });
    }

    function setBarcodeMessage(key, params = {}) {
        state.barcodeMessage = { key, params };
        document.getElementById('barcode-box').textContent = t(key, params);
    }

    function setSessionMessage(key, params = {}) {
        state.sessionMessage = { key, params };
        document.getElementById('session-status').textContent = t(key, params);
    }

    function setA0Message(key, params = {}) {
        state.a0Message = { key, params };
        document.getElementById('a0-review-status').textContent = t(key, params);
    }

    function setAssessmentMessage(key, params = {}) {
        state.assessmentMessage = { key, params };
        document.getElementById('assessment-summary').textContent = t(key, params);
    }

    function roleLabel(role) {
        return t(`image_role_${role}`) || role;
    }

    function updateRoleOptions() {
        const roleInput = document.getElementById('image-role');
        Array.from(roleInput.options).forEach((option) => {
            option.textContent = roleLabel(option.value);
        });
    }

    function assessmentTile(titleKey, status, label, metric) {
        const m = metric === null || metric === undefined ? '-' : (Number.isFinite(Number(metric)) ? Number(metric).toFixed(2) : metric);
        return `
            <div class="assessment-tile">
                <strong>${t(titleKey)}</strong>
                <div class="assessment-main">${escapeHtml(status || t('unknown'))}</div>
                <div class="assessment-sub">${escapeHtml(label || '-')} &bull; ${escapeHtml(m)}</div>
            </div>
        `;
    }

    function renderAssessment(assessment) {
        if (!assessment) {
            if (state.assessmentMessage) setAssessmentMessage(state.assessmentMessage.key, state.assessmentMessage.params);
            return;
        }

        state.assessment = assessment;
        state.assessmentMessage = null;
        const summaryEl = document.getElementById('assessment-summary');
        const gridEl = document.getElementById('assessment-grid');
        const dims = assessment.dimensions || {};

        summaryEl.textContent = t('assessment_summary', {
            summary: assessment.summary || '-',
            action: assessment.recommended_action || '-',
            completeness: assessment.completeness || '-',
            date: formatDate(assessment.valid_until)
        });

        gridEl.innerHTML = [
            assessmentTile('dim_fruit', dims.fruit?.status, dims.fruit?.label, dims.fruit?.confidence),
            assessmentTile('dim_disease', dims.disease?.risk_level || dims.disease?.status, dims.disease?.label, dims.disease?.confidence),
            assessmentTile('dim_growth', dims.growth?.status, dims.growth?.label, dims.growth?.vigor_index),
            assessmentTile('dim_uav', dims.uav?.status, t('shift_value', { value: dims.uav?.center_shift ?? '-' }), dims.uav?.confidence)
        ].join('');
    }

    async function loadAssessment(treeCode) {
        try {
            const res = await fetch(`/api/v1/trees/${encodeURIComponent(treeCode)}/assessment`);
            const data = await res.json();
            if (data.status !== 'ok') {
                state.assessment = null;
                setAssessmentMessage('assessment_unavailable');
                return;
            }
            renderAssessment(data.assessment);
        } catch (e) {
            state.assessment = null;
            setAssessmentMessage('assessment_failed', { message: e.message });
        }
    }

    async function loadBarcode(treeCode) {
        try {
            const res = await fetch(`/api/v1/trees/${encodeURIComponent(treeCode)}/barcode`);
            const data = await res.json();
            if (data.status === 'ok') {
                setBarcodeMessage('barcode_value', { value: data.barcode_value });
            } else {
                setBarcodeMessage('barcode_unavailable');
            }
        } catch (e) {
            setBarcodeMessage('barcode_failed');
        }
    }

    function infoRow(labelKey, value, valueHtml = false) {
        const renderedValue = valueHtml ? value : escapeHtml(value ?? '-');
        return `<div class="info-row"><span class="info-label">${t(labelKey)}</span><span class="info-value">${renderedValue}</span></div>`;
    }

    function renderBasicInfo() {
        const tree = state.tree;
        if (!tree) return;
        const statusClass = `badge-${tree.current_status}`;
        document.getElementById('basic-info').innerHTML = [
            infoRow('tree_code', tree.tree_code),
            infoRow('species', tree.species),
            infoRow('status', `<span class="badge ${statusClass}">${escapeHtml(tree.current_status)}</span>`, true),
            infoRow('barcode_binding', tree.barcode_value || '-'),
            infoRow('verified', tree.manual_verified ? t('yes') : t('no')),
            infoRow('plantation', tree.plantation_name || '-'),
            infoRow('created', formatDate(tree.created_at))
        ].join('');
    }

    function renderLocationInfo() {
        const tree = state.tree;
        if (!tree) return;
        document.getElementById('location-info').innerHTML = [
            infoRow('coordinate_x', tree.coordinate_x ?? '-'),
            infoRow('coordinate_y', tree.coordinate_y ?? '-'),
            infoRow('crown_center_x', tree.crown_center_x ?? '-'),
            infoRow('crown_center_y', tree.crown_center_y ?? '-'),
            infoRow('source_ortho_id', tree.source_orthomosaic_id ?? '-'),
            infoRow('block', tree.block_id || '-')
        ].join('');
    }

    function renderTimeline() {
        const el = document.getElementById('timeline-content');
        if (state.timeline === null) {
            el.innerHTML = `<div class="timeline-empty">${t('timeline_failed')}</div>`;
            return;
        }
        if (!state.timeline.length) {
            el.innerHTML = `<div class="timeline-empty">${t('no_history_records')}</div>`;
            return;
        }

        el.innerHTML = state.timeline.map((item) => `
            <div class="timeline-item">
                <strong>${escapeHtml(item.mission_name || '-')}</strong>
                <span style="color:rgba(255,255,255,0.4); margin-left:8px;">${formatDate(item.mission_date || item.created_at)}</span>
                <div style="margin-top:4px; font-size:0.88rem; color:#9ca3af;">
                    ${t('detected')}: (${item.detected_x?.toFixed(2) ?? '-'}, ${item.detected_y?.toFixed(2) ?? '-'})
                    &bull; ${t('shift')}: ${item.center_shift?.toFixed(3) ?? '-'}
                    &bull; ${t('confidence')}: ${item.match_confidence?.toFixed(2) ?? '-'}
                </div>
            </div>
        `).join('');
    }

    function renderActions() {
        const tree = state.tree;
        if (!tree) return;
        const bar = document.getElementById('action-bar');
        const statuses = ['active', 'dead', 'removed', 'replanted'];
        bar.innerHTML = '';

        statuses.forEach((status) => {
            if (status === tree.current_status) return;
            const btn = document.createElement('button');
            btn.className = 'btn-status' + (status === 'dead' || status === 'removed' ? ' danger' : '');
            btn.textContent = t('mark_as', { status });
            btn.addEventListener('click', async () => {
                if (!confirm(t('change_status_confirm', { status }))) return;
                try {
                    const res = await fetch(`/api/v1/trees/${encodeURIComponent(state.code)}/status`, {
                        method: 'PUT',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ status })
                    });
                    if (res.ok) {
                        window.location.reload();
                    } else {
                        const err = await res.json();
                        alert(t('failed_message', { message: unknownError(err.message) }));
                    }
                } catch (e) {
                    alert(t('error_prefix', { message: e.message }));
                }
            });
            bar.appendChild(btn);
        });
    }

    function bindSessionActions() {
        const startBtn = document.getElementById('btn-start-session');
        const uploadBtn = document.getElementById('btn-upload-session-image');
        const fileInput = document.getElementById('session-image');
        const roleInput = document.getElementById('image-role');

        function hasSelectedFile() {
            return Boolean(fileInput.files && fileInput.files.length);
        }

        updateSessionControls = function () {
            const hasFile = hasSelectedFile();
            startBtn.disabled = state.sessionCreating || state.sessionUploading || Boolean(state.sessionId);
            uploadBtn.disabled = state.sessionCreating || state.sessionUploading || !state.sessionId || !hasFile || Boolean(state.a0);
        };

        fileInput.addEventListener('change', () => {
            if (!state.sessionId && hasSelectedFile()) {
                setSessionMessage('start_session_before_upload');
            }
            updateSessionControls();
        });

        startBtn.addEventListener('click', async () => {
            if (!state.tree?.id) return;
            if (state.sessionId) {
                setSessionMessage('session_already_active', { code: state.sessionCode || state.sessionId });
                updateSessionControls();
                return;
            }
            setSessionMessage('creating_session');
            state.sessionCreating = true;
            updateSessionControls();
            try {
                const res = await fetch(`/api/v1/trees/${state.tree.id}/sessions`, { method: 'POST' });
                const data = await res.json();
                if (data.status === 'ok') {
                    state.sessionId = data.session.id;
                    state.sessionCode = data.session.session_code;
                    setSessionMessage('active_session', { code: data.session.session_code });
                    await loadTreeSessions(state.tree.id);
                } else {
                    setSessionMessage('session_failed', { message: unknownError(data.message) });
                }
            } catch (e) {
                setSessionMessage('session_error', { message: e.message });
            } finally {
                state.sessionCreating = false;
                updateSessionControls();
            }
        });

        uploadBtn.addEventListener('click', async () => {
            const file = fileInput.files[0];
            const role = roleInput.value;
            if (!state.sessionId || !file) {
                setSessionMessage(!state.sessionId ? 'start_session_before_upload' : 'upload_select_image');
                updateSessionControls();
                return;
            }

            const form = new FormData();
            form.append('image_role', role);
            form.append('file', file);
            setSessionMessage('uploading_session_image');
            state.sessionUploading = true;
            updateSessionControls();

            try {
                const res = await fetch(`/api/v1/sessions/${state.sessionId}/images?image_role=${encodeURIComponent(role)}`, {
                    method: 'POST',
                    body: form
                });
                const data = await res.json();
                if (data.status === 'ok') {
                    setSessionMessage(
                        data.requires_confirmation ? 'uploaded_needs_a0' : 'uploaded_image',
                        { role: roleLabel(data.image.image_role) }
                    );
                    fileInput.value = '';
                    if (data.requires_confirmation) {
                        showA0Review(data.image, data.analysis);
                    } else {
                        await loadTreeSessions(state.tree.id);
                        if (state.tree?.tree_code) await loadAssessment(state.tree.tree_code);
                    }
                } else {
                    setSessionMessage('upload_failed', { message: unknownError(data.message) });
                }
            } catch (e) {
                setSessionMessage('upload_error', { message: e.message });
            } finally {
                state.sessionUploading = false;
                updateSessionControls();
            }
        });

        setSessionMessage(state.sessionId ? 'active_session' : 'no_active_session', { code: state.sessionCode || state.sessionId });
        updateSessionControls();
    }

    function showA0Review(image, analysis) {
        const candidates = analysis?.metadata?.a0_candidates || [];
        state.a0 = {
            image,
            analysis,
            candidates,
            selected: new Set(candidates.map((candidate) => candidate.candidate_id))
        };

        document.getElementById('a0-review').style.display = 'grid';
        const stage = document.getElementById('a0-stage');
        stage.innerHTML = `<img id="a0-review-img" src="${fixImageUrl(image.image_url, image.upload_id)}" alt="A0 review image">`;
        setA0Message(
            analysis?.metadata?.route_status === 'needs_user_confirmation' ? 'a0_remove_hint' : 'a0_route_status',
            { status: analysis?.metadata?.route_status || t('unknown') }
        );
        renderA0Boxes();
        wireA0Buttons();
        updateSessionControls();
    }

    function renderA0Boxes() {
        if (!state.a0) return;
        const stage = document.getElementById('a0-stage');
        stage.querySelectorAll('.a0-box').forEach((el) => el.remove());

        state.a0.candidates.forEach((candidate) => {
            const geometry = candidate.geometry || {};
            if (geometry.type !== 'bbox') return;
            const box = document.createElement('button');
            box.type = 'button';
            box.className = 'a0-box' + (state.a0.selected.has(candidate.candidate_id) ? '' : ' rejected');
            box.style.left = `${Number(geometry.x || 0) * 100}%`;
            box.style.top = `${Number(geometry.y || 0) * 100}%`;
            box.style.width = `${Number(geometry.w || 0) * 100}%`;
            box.style.height = `${Number(geometry.h || 0) * 100}%`;
            box.innerHTML = `<span>${escapeHtml(candidate.label)} ${(Number(candidate.confidence || 0) * 100).toFixed(0)}%</span>`;
            box.addEventListener('click', () => {
                if (state.a0.selected.has(candidate.candidate_id)) {
                    state.a0.selected.delete(candidate.candidate_id);
                } else {
                    state.a0.selected.add(candidate.candidate_id);
                }
                renderA0Boxes();
            });
            stage.appendChild(box);
        });

        renderA0Counts();
    }

    function renderA0Counts() {
        if (!state.a0) return;
        const selected = state.a0.selected.size;
        const rejected = Math.max(state.a0.candidates.length - selected, 0);
        document.getElementById('a0-review-counts').textContent = t('selected_rejected', { selected, rejected });
        document.getElementById('btn-confirm-a0').disabled = selected === 0;
    }

    function wireA0Buttons() {
        const confirmBtn = document.getElementById('btn-confirm-a0');
        const cancelBtn = document.getElementById('btn-cancel-a0');

        confirmBtn.onclick = async () => {
            if (!state.a0?.selected.size) {
                setA0Message('select_candidate_warning');
                return;
            }
            setA0Message('creating_masked_analysis');
            try {
                const res = await fetch(`/api/v1/sessions/${state.sessionId}/images/${state.a0.image.id}/confirm`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ selected_candidate_ids: Array.from(state.a0.selected) })
                });
                const data = await res.json();
                if (data.status === 'ok') {
                    document.getElementById('a0-review').style.display = 'none';
                    state.a0 = null;
                    updateSessionControls();
                    await loadTreeSessions(state.tree.id);
                    if (state.tree?.tree_code) await loadAssessment(state.tree.tree_code);
                } else {
                    setA0Message('confirmation_failed', { message: unknownError(data.message) });
                }
            } catch (e) {
                setA0Message('confirmation_error', { message: e.message });
            }
        };

        cancelBtn.onclick = () => {
            document.getElementById('a0-review').style.display = 'none';
            state.a0 = null;
            updateSessionControls();
        };
    }

    async function loadSessionImages(sessionId) {
        if (!sessionId) return;
        try {
            const res = await fetch(`/api/v1/sessions/${sessionId}/images`);
            const data = await res.json();
            state.sessionImages = data.status === 'ok' ? (data.images || []) : [];
            renderSessionImages();
        } catch (e) {
            console.error('Failed to load session images', e);
        }
    }

    async function loadTreeSessions(treeId) {
        if (!treeId) return;
        try {
            const res = await fetch(`/api/v1/trees/${treeId}/sessions`);
            const data = await res.json();
            if (data.status !== 'ok') {
                state.sessions = [];
                state.sessionImages = [];
                state.sessionId = null;
                state.sessionCode = null;
                setSessionMessage('session_failed', { message: unknownError(data.message) });
                renderSessionImages();
                updateSessionControls();
                return;
            }

            state.sessions = data.sessions || [];
            const active = data.active_session || state.sessions.find((session) => session.status === 'active') || null;
            state.sessionId = active?.id || null;
            state.sessionCode = active?.session_code || null;
            state.sessionImages = state.sessions.flatMap((session) =>
                (session.images || []).map((image) => ({
                    ...image,
                    session_code: image.session_code || session.session_code
                }))
            ).sort((a, b) => new Date(a.created_at || 0) - new Date(b.created_at || 0));

            if (state.sessionId) {
                setSessionMessage('active_session', { code: state.sessionCode || state.sessionId });
            } else {
                setSessionMessage('no_active_session');
            }
            renderSessionImages();
            updateSessionControls();
        } catch (e) {
            setSessionMessage('session_error', { message: e.message });
            updateSessionControls();
        }
    }

    function renderSessionImages() {
        const el = document.getElementById('session-result');
        if (!state.sessionImages.length) {
            el.textContent = t('session_result_empty');
            return;
        }

        const latest = state.sessionImages[state.sessionImages.length - 1];
        el.innerHTML = `
            <div class="session-image-grid">
                ${state.sessionImages.map((img) => `
                    <div class="session-image-card">
                        <img src="${fixImageUrl(img.image_url, img.upload_id)}" alt="${escapeHtml(roleLabel(img.image_role))} evidence">
                        <div class="session-image-role">${escapeHtml(roleLabel(img.image_role))}</div>
                        <div class="session-image-meta">${escapeHtml(img.metadata?.route_status || img.mock_analysis?.metadata?.route_status || 'analysis')}</div>
                        <div class="session-image-meta">${formatDate(img.created_at)}</div>
                    </div>
                `).join('')}
            </div>
            <div class="analysis-detail">
                <h4>${t('latest_analysis_detail')}</h4>
                <pre>${escapeHtml(JSON.stringify(latest.mock_analysis, null, 2))}</pre>
            </div>
        `;
    }

    if (!code && barcode) {
        setLoading('looking_up_barcode');
        try {
            const res = await fetch(`/api/v1/trees/by-barcode/${encodeURIComponent(barcode)}`);
            const data = await res.json();
            if (data.status === 'ok' && data.tree?.tree_code) {
                code = data.tree.tree_code;
            }
        } catch (e) {
            setLoading('barcode_lookup_failed', { message: e.message });
            return;
        }
    }

    if (!code) {
        setLoading('missing_tree_identifier');
        return;
    }

    state.code = code;
    updateProfileTitle();
    updateRoleOptions();

    try {
        const res = await fetch(`/api/v1/trees/${encodeURIComponent(code)}`);
        if (!res.ok) {
            setLoading('tree_not_found', { code, status: res.status });
            return;
        }
        const data = await res.json();
        state.tree = data.tree;

        renderBasicInfo();
        renderLocationInfo();
        renderActions();
        bindSessionActions();
        await Promise.all([loadBarcode(code), loadAssessment(code), loadTreeSessions(state.tree.id)]);

        loading.style.display = 'none';
        content.style.display = 'block';
    } catch (e) {
        setLoading('error_loading_tree', { message: e.message });
        return;
    }

    try {
        const tlRes = await fetch(`/api/v1/trees/${encodeURIComponent(code)}/timeline`);
        const tlData = await tlRes.json();
        state.timeline = tlData.timeline || [];
        renderTimeline();
    } catch (e) {
        state.timeline = null;
        renderTimeline();
    }

    document.addEventListener('op:i18n-change', () => {
        updateProfileTitle();
        updateRoleOptions();
        if (state.loadingMessage) loading.textContent = t(state.loadingMessage.key, state.loadingMessage.params);
        if (state.barcodeMessage) setBarcodeMessage(state.barcodeMessage.key, state.barcodeMessage.params);
        setSessionMessage(state.sessionMessage.key, state.sessionMessage.params);
        setA0Message(state.a0Message.key, state.a0Message.params);
        renderBasicInfo();
        renderLocationInfo();
        renderActions();
        renderAssessment(state.assessment);
        renderTimeline();
        renderSessionImages();
        if (state.a0) {
            renderA0Boxes();
            renderA0Counts();
        }
    });
});
