document.addEventListener('DOMContentLoaded', () => {
    const $ = (id) => document.getElementById(id);
    const btnCreateMission = $('btn-create-mission');
    const btnRegisterOrtho = $('btn-register-ortho');
    const btnAiDetections = $('btn-ai-detections');
    const btnAutoMatch = $('btn-auto-match');
    const btnViewOrtho = $('btn-view-ortho');
    const missionStatus = $('mission-status');
    const orthoStatus = $('ortho-status');
    const detectionStatus = $('detection-status');
    const detectionList = $('detection-list');
    const treeList = $('tree-list');
    const matchStatus = $('match-status');
    const matchReviewList = $('match-review-list');

    let missionId = null;
    let orthoId = null;
    let detectionsCache = [];
    let matchReviewsCache = [];
    let confirmedTreeCodes = [];

    const statusMemory = new Map();

    function t(key, params = {}) {
        if (window.OP_I18N) return window.OP_I18N.t(key, params);
        return Object.entries(params).reduce((out, [name, value]) => out.replaceAll(`{${name}}`, value), key);
    }

    function unknownError(message) {
        return message || t('unknown_error');
    }

    function setStatus(el, key, params = {}) {
        statusMemory.set(el, { key, params });
        el.textContent = t(key, params);
    }

    function setStatusText(el, text) {
        statusMemory.delete(el);
        el.textContent = text;
    }

    function renderRememberedStatuses() {
        statusMemory.forEach(({ key, params }, el) => {
            el.textContent = t(key, params);
        });
    }

    function buildOrthoLink() {
        btnViewOrtho.style.display = orthoId ? 'block' : 'none';
        if (orthoId) btnViewOrtho.href = `ortho_viewer.html?ortho_id=${orthoId}`;
    }

    async function parseJsonResponse(res) {
        const data = await res.json().catch(() => ({}));
        if (!res.ok || data.status === 'error') {
            throw new Error(unknownError(data.message || res.statusText));
        }
        return data;
    }

    function readImageDimensions(url) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            img.onload = () => resolve({
                width: img.naturalWidth || img.width,
                height: img.naturalHeight || img.height
            });
            img.onerror = () => reject(new Error(t('error_prefix', { message: 'image load failed' })));
            img.src = url;
        });
    }

    async function init() {
        try {
            const res = await fetch('/api/v1/uav/missions');
            const data = await res.json();
            const missions = data.missions || [];

            if (!missions.length) return;

            const lastMission = missions[missions.length - 1];
            missionId = lastMission.id;
            setStatus(missionStatus, 'current_mission', { name: lastMission.mission_name, id: missionId });
            btnRegisterOrtho.disabled = false;
            btnAutoMatch.disabled = false;

            const orthoRes = await fetch(`/api/v1/uav/missions/${missionId}/orthomosaic`);
            if (!orthoRes.ok) return;

            const orthoData = await orthoRes.json();
            if (orthoData && orthoData.id) {
                orthoId = orthoData.id;
                setStatus(orthoStatus, 'ortho_current', { url: orthoData.image_url, id: orthoId });
                btnAiDetections.disabled = false;
                buildOrthoLink();
                await fetchDetections();
            }
        } catch (e) {
            console.log('Init state recovery skipped or failed', e);
        }
    }

    btnCreateMission.addEventListener('click', async () => {
        const plantationName = $('plantation-name').value || t('default_plantation');
        const missionName = $('mission-name').value || t('unnamed_mission');

        try {
            const res = await fetch('/api/v1/uav/missions', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    plantation_id: 0,
                    plantation_name: plantationName,
                    mission_name: missionName
                })
            });
            const data = await res.json();
            missionId = data.mission_id;
            const plantationId = data.plantation_id;
            setStatus(missionStatus, 'mission_created', { name: missionName, id: missionId, plantationId });
            btnRegisterOrtho.disabled = false;
            btnAutoMatch.disabled = false;
            setStatus(orthoStatus, 'status_ready_register');
        } catch (e) {
            setStatus(missionStatus, 'error_prefix', { message: e.message });
        }
    });

    btnRegisterOrtho.addEventListener('click', async () => {
        const orthoUrl = $('ortho-url').value;
        if (!orthoUrl) {
            alert(t('image_url_required'));
            return;
        }

        try {
            const dimensions = await readImageDimensions(orthoUrl).catch(() => ({
                width: 658,
                height: 438
            }));
            const res = await fetch(`/api/v1/uav/missions/${missionId}/orthomosaic`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    width: dimensions.width,
                    height: dimensions.height,
                    resolution: 0.1,
                    image_url: orthoUrl
                })
            });
            const data = await parseJsonResponse(res);
            orthoId = data.orthomosaic_id;
            setStatus(orthoStatus, 'orthomosaic_registered', { id: orthoId });
            btnAiDetections.disabled = false;
            buildOrthoLink();
        } catch (e) {
            setStatus(orthoStatus, 'error_prefix', { message: e.message });
        }
    });

    btnAiDetections.addEventListener('click', async () => {
        try {
            setStatus(detectionStatus, 'running_detection');
            const tileRes = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/tiles`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tile_size: 1024, tile_overlap: 0.15 })
            });
            await parseJsonResponse(tileRes);

            const res = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/detect-palms`, { method: 'POST' });
            const data = await parseJsonResponse(res);
            setStatus(detectionStatus, 'detections_created_from_tiles', {
                count: data.detections_created || 0,
                tiles: data.tiles_processed || 0
            });
            await fetchDetections();
        } catch (e) {
            setStatus(detectionStatus, 'detection_failed', { message: e.message });
        }
    });

    btnAutoMatch.addEventListener('click', async () => {
        try {
            setStatus(matchStatus, 'matching_existing');
            const res = await fetch(`/api/v1/uav/missions/${missionId}/match-existing-trees`, { method: 'POST' });
            const data = await res.json();
            if (data.status === 'ok') {
                setStatus(matchStatus, 'match_summary', {
                    auto: data.auto_matched,
                    ambiguous: data.ambiguous,
                    unmatched: data.unmatched
                });
                if (data.ambiguous > 0) await loadMatchReview();
                await refreshTreesAndDetections();
            } else {
                setStatus(matchStatus, 'match_failed', { message: unknownError(data.message) });
            }
        } catch (e) {
            setStatus(matchStatus, 'error_prefix', { message: e.message });
        }
    });

    async function loadMatchReview() {
        try {
            const res = await fetch(`/api/v1/uav/missions/${missionId}/match-review`);
            const data = await res.json();
            matchReviewsCache = data.reviews || [];
            renderMatchReviews(matchReviewsCache);
        } catch (e) {
            console.error('match review load failed', e);
        }
    }

    function renderMatchReviews(reviews) {
        matchReviewList.innerHTML = '';
        if (!reviews.length) {
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.textContent = t('no_ambiguous_matches');
            matchReviewList.appendChild(empty);
            return;
        }

        reviews.forEach((review) => {
            const item = document.createElement('div');
            item.className = 'detection-item';

            const body = document.createElement('div');
            const title = document.createElement('span');
            title.textContent = t('detection_candidate_label', {
                id: review.detection_id,
                confidence: Number(review.confidence || 0).toFixed(2)
            });
            body.appendChild(title);

            const candidates = document.createElement('div');
            candidates.style.marginTop = '4px';
            const label = document.createElement('span');
            label.textContent = t('candidate_list_label');
            candidates.appendChild(label);

            (review.candidates || []).forEach((candidate) => {
                const option = document.createElement('div');
                option.className = 'match-candidate';
                option.tabIndex = 0;
                option.role = 'button';
                option.textContent = t('tree_candidate_label', {
                    code: candidate.tree_code,
                    distance: (Number(candidate.distance_pixels || 0) * 0.05).toFixed(2)
                });
                option.addEventListener('click', () => matchToTree(review.detection_id, candidate.tree_id, item));
                option.addEventListener('keydown', (event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        matchToTree(review.detection_id, candidate.tree_id, item);
                    }
                });
                candidates.appendChild(option);
            });

            body.appendChild(candidates);
            item.appendChild(body);
            matchReviewList.appendChild(item);
        });
    }

    async function matchToTree(detId, treeId, element) {
        try {
            const res = await fetch(`/api/v1/uav/detections/${detId}/match-to-tree`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tree_id: treeId })
            });
            const data = await res.json();
            if (data.status === 'ok') {
                element.remove();
                setStatus(matchStatus, 'matched_detection', { id: detId, code: data.tree_code });
            } else {
                alert(t('match_failed', { message: unknownError(data.message) }));
            }
        } catch (e) {
            alert(t('match_error', { message: e.message }));
        }
    }

    async function refreshTreesAndDetections() {
        await fetchDetections();
    }

    async function fetchDetections() {
        if (!orthoId) return;
        try {
            const res = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/detections`);
            const data = await res.json();
            detectionsCache = data.detections || [];
            renderDetections(detectionsCache);
        } catch (e) {
            console.error('fetch detections failed', e);
        }
    }

    function renderDetections(detections) {
        detectionList.innerHTML = '';
        const pending = detections.filter((d) => d.review_status === 'pending');

        pending.forEach((detection) => {
            const item = document.createElement('div');
            item.className = 'detection-item';

            const title = document.createElement('span');
            title.textContent = t('detection_candidate_label', {
                id: detection.id,
                confidence: Number(detection.confidence || 0).toFixed(2)
            });

            const actions = document.createElement('div');
            actions.className = 'detection-actions';

            const confirmBtn = document.createElement('button');
            confirmBtn.className = 'btn success';
            confirmBtn.textContent = t('confirm');
            confirmBtn.addEventListener('click', () => confirmDetection(detection.id, item));

            const rejectBtn = document.createElement('button');
            rejectBtn.className = 'btn danger';
            rejectBtn.textContent = t('reject');
            rejectBtn.addEventListener('click', () => rejectDetection(detection.id, item));

            actions.append(confirmBtn, rejectBtn);
            item.append(title, actions);
            detectionList.appendChild(item);
        });

        if (!pending.length) {
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.textContent = t('no_pending_detections');
            detectionList.appendChild(empty);
        }
    }

    function renderConfirmedTrees() {
        treeList.innerHTML = '';
        if (!confirmedTreeCodes.length) {
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.textContent = t('confirmed_tree_assets_empty');
            treeList.appendChild(empty);
            return;
        }

        confirmedTreeCodes.forEach((code) => {
            const item = document.createElement('div');
            item.className = 'tree-item';

            const label = document.createElement('span');
            label.append(`${t('tree_asset_link')} `);
            const link = document.createElement('a');
            link.href = `tree_profile.html?code=${encodeURIComponent(code)}`;
            link.textContent = code;
            label.appendChild(link);

            const badge = document.createElement('span');
            badge.className = 'status-pill status-ok';
            badge.textContent = t('status_confirmed');

            item.append(label, badge);
            treeList.appendChild(item);
        });
    }

    async function confirmDetection(id, element) {
        try {
            const res = await fetch(`/api/v1/uav/detections/${id}/confirm`, { method: 'POST' });
            const data = await res.json();
            if (data.status !== 'ok') {
                alert(t('confirm_failed', { message: unknownError(data.message) }));
                return;
            }
            element.remove();
            if (data.tree_code && !confirmedTreeCodes.includes(data.tree_code)) {
                confirmedTreeCodes.push(data.tree_code);
            }
            renderConfirmedTrees();
            await fetchDetections();
        } catch (e) {
            alert(t('confirm_failed', { message: e.message || t('unknown_error') }));
        }
    }

    async function rejectDetection(id, element) {
        try {
            const res = await fetch(`/api/v1/uav/detections/${id}/reject`, { method: 'POST' });
            const data = await res.json().catch(() => ({}));
            if (data.status && data.status !== 'ok') {
                alert(t('reject_failed', { message: unknownError(data.message) }));
                return;
            }
            element.remove();
            await fetchDetections();
        } catch (e) {
            alert(t('reject_failed', { message: e.message || t('unknown_error') }));
        }
    }

    document.addEventListener('op:i18n-change', () => {
        renderRememberedStatuses();
        renderDetections(detectionsCache);
        renderMatchReviews(matchReviewsCache);
        renderConfirmedTrees();
    });

    window.confirmDetection = confirmDetection;
    window.rejectDetection = rejectDetection;
    window.matchToTree = matchToTree;

    init();
});
