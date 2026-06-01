(function () {
    'use strict';

    var orthoId = getQueryParam('ortho_id');
    var map = null;
    var imageOverlay = null;
    var detectionMarkers = {};
    var selectedDetection = null;
    var detectionsCache = [];
    var selectedDetectionIds = new Set();
    var bulkConfirming = false;
    var selectionMode = false;
    var manualMode = false;
    var orthoWidth = 0;
    var orthoHeight = 0;
    var orthoResolution = 0;
    var lastStatus = { key: 'waiting_orthomosaic', params: {} };

    var STATUS_COLORS = {
        pending: '#f59e0b',
        confirmed: '#10b981',
        rejected: '#ef4444',
        corrected: '#6366f1'
    };
    var BULK_SELECTED_FILL = '#0284c7';
    var BULK_SELECTED_STROKE = '#e0f2fe';

    function t(key, params) {
        if (window.OP_I18N) return window.OP_I18N.t(key, params || {});
        return Object.entries(params || {}).reduce(function (out, entry) {
            return out.replaceAll('{' + entry[0] + '}', entry[1]);
        }, key);
    }

    function unknownError(message) {
        return message || t('unknown_error');
    }

    function getQueryParam(name) {
        var params = new URLSearchParams(window.location.search);
        return params.get(name);
    }

    function apiGet(path) {
        return fetch('/api/v1/uav' + path).then(function (r) { return r.json(); });
    }

    function apiPost(path, body) {
        return fetch('/api/v1/uav' + path, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body || {})
        }).then(function (r) { return r.json(); });
    }

    function setStatus(key, params) {
        lastStatus = { key: key, params: params || {} };
        document.getElementById('action-status').textContent = t(key, params || {});
    }

    function pendingDetections() {
        return detectionsCache.filter(function (det) {
            return det.review_status === 'pending';
        });
    }

    function pruneSelection() {
        var pendingIds = new Set(pendingDetections().map(function (det) {
            return String(det.id);
        }));
        Array.from(selectedDetectionIds).forEach(function (id) {
            if (!pendingIds.has(String(id))) selectedDetectionIds.delete(String(id));
        });
    }

    function updateBulkControls() {
        var pending = pendingDetections();
        pruneSelection();
        if (!pending.length) selectionMode = false;
        var selected = selectedDetectionIds.size;
        document.getElementById('bulk-selection-count').textContent = selected + ' / ' + pending.length;
        var modeBtn = document.getElementById('btn-selection-mode');
        modeBtn.disabled = bulkConfirming || pending.length === 0;
        modeBtn.classList.toggle('active', selectionMode);
        modeBtn.setAttribute('aria-pressed', selectionMode ? 'true' : 'false');
        modeBtn.textContent = selectionMode ? t('selection_mode_on') : t('selection_mode');
        document.getElementById('btn-select-all-pending').disabled = bulkConfirming || pending.length === 0;
        document.getElementById('btn-clear-selection').disabled = bulkConfirming || selected === 0;
        document.getElementById('btn-confirm-selected').disabled = bulkConfirming || selected === 0;
    }

    function toggleDetectionSelection(detId, focusDet) {
        var id = String(detId);
        if (selectedDetectionIds.has(id)) {
            selectedDetectionIds.delete(id);
        } else {
            selectedDetectionIds.add(id);
        }
        renderDetections(detectionsCache);
        if (focusDet || selectedDetection) selectDetection(focusDet || selectedDetection, false);
        updateBulkControls();
    }

    function toggleSelectionMode() {
        selectionMode = !selectionMode;
        setStatus(selectionMode ? 'selection_mode_enabled' : 'selection_mode_disabled');
        updateBulkControls();
    }

    function findDetectionById(detId) {
        return detectionsCache.find(function (det) {
            return String(det.id) === String(detId);
        });
    }

    function markerOptions(det, focused) {
        var isSelected = selectedDetectionIds.has(String(det.id));
        var statusColor = STATUS_COLORS[det.review_status] || '#94a3b8';
        var opacity = det.review_status === 'confirmed' ? 0.6 : 0.85;

        if (isSelected) {
            return {
                radius: focused ? 23 : 21,
                style: {
                    color: BULK_SELECTED_STROKE,
                    fillColor: BULK_SELECTED_FILL,
                    fillOpacity: 0.96,
                    opacity: 1,
                    weight: focused ? 6 : 4,
                    dashArray: '4 3'
                }
            };
        }

        if (focused) {
            return {
                radius: 18,
                style: {
                    color: '#f8fafc',
                    fillColor: statusColor,
                    fillOpacity: 1,
                    opacity: 1,
                    weight: 4,
                    dashArray: null
                }
            };
        }

        return {
            radius: 15,
            style: {
                color: statusColor,
                fillColor: statusColor,
                fillOpacity: opacity,
                opacity: 1,
                weight: 2,
                dashArray: null
            }
        };
    }

    function applyMarkerStyle(det, focused) {
        var marker = detectionMarkers[det.id];
        if (!marker) return;
        var options = markerOptions(det, focused);
        marker.setStyle(options.style);
        if (marker.setRadius) marker.setRadius(options.radius);
    }

    function updateManualButton() {
        document.getElementById('btn-manual-mode').textContent = manualMode
            ? t('cancel_manual_mode')
            : t('add_tree_manually');
    }

    function generatePlaceholderImage(w, h) {
        var canvas = document.createElement('canvas');
        canvas.width = Math.min(w, 2000);
        canvas.height = Math.min(h, 2000);
        var scaleX = canvas.width / w;
        var scaleY = canvas.height / h;
        var ctx = canvas.getContext('2d');

        ctx.fillStyle = '#1a2e1a';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        var gridSize = 100 * scaleX;
        ctx.strokeStyle = 'rgba(42, 90, 42, 0.6)';
        ctx.lineWidth = 1;
        for (var x = 0; x < canvas.width; x += gridSize) {
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, canvas.height);
            ctx.stroke();
        }
        for (var y = 0; y < canvas.height; y += gridSize) {
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(canvas.width, y);
            ctx.stroke();
        }

        ctx.fillStyle = 'rgba(255, 255, 255, 0.1)';
        ctx.font = Math.max(12, gridSize * 0.2) + 'px monospace';
        for (var labelX = 0; labelX < w; labelX += 200) {
            for (var labelY = 0; labelY < h; labelY += 200) {
                ctx.fillText(labelX + ',' + labelY, labelX * scaleX + 2, labelY * scaleY + gridSize * 0.5);
            }
        }

        return canvas.toDataURL('image/png');
    }

    function initMap(w, h, realImgUrl) {
        orthoWidth = w;
        orthoHeight = h;

        if (map) {
            map.remove();
        }

        map = L.map('map', {
            crs: L.CRS.Simple,
            minZoom: -3,
            maxZoom: 6,
            zoomSnap: 0.25,
            zoomDelta: 0.5,
            attributionControl: false
        });

        var imgUrl = realImgUrl || generatePlaceholderImage(w, h);
        var bounds = [[0, 0], [h, w]];
        imageOverlay = L.imageOverlay(imgUrl, bounds).addTo(map);
        map.fitBounds(bounds);

        map.on('mousemove', function (e) {
            document.getElementById('coords-display').textContent = t('coords_xy', {
                x: e.latlng.lng.toFixed(1),
                y: e.latlng.lat.toFixed(1)
            });
        });

        map.on('click', function (e) {
            if (manualMode) {
                handleManualPlace(e.latlng.lng, e.latlng.lat);
            }
        });

        map.on('contextmenu', function () {
            if (manualMode) {
                disableManualMode();
            }
        });
    }

    function loadOrthoInfo() {
        apiGet('/orthomosaics/' + orthoId).then(function (data) {
            if (data.status !== 'ok' || !data.orthomosaic) {
                setStatus('failed_load_ortho');
                return;
            }
            var o = data.orthomosaic;
            orthoResolution = o.resolution || 0.05;
            document.getElementById('info-id').textContent = o.id;
            document.getElementById('info-mission').textContent = o.mission_id;
            document.getElementById('info-size').textContent = o.width + ' x ' + o.height;
            document.getElementById('info-resolution').textContent = (o.resolution * 100).toFixed(1) + ' cm/px';

            initMap(o.width, o.height, o.image_url);
            document.getElementById('btn-detect-palms').disabled = false;
            loadDetections();
        }).catch(function (err) {
            setStatus('error_loading_ortho', { message: err.message });
        });
    }

    function loadDetections() {
        apiGet('/orthomosaics/' + orthoId + '/detections').then(function (data) {
            if (data.status !== 'ok') {
                setStatus('failed_load_detections');
                return;
            }
            detectionsCache = data.detections || [];
            pruneSelection();
            document.getElementById('info-det-count').textContent = detectionsCache.length;
            renderDetections(detectionsCache);
            updateBulkControls();
            setStatus('detections_loaded', { count: detectionsCache.length });
        }).catch(function (err) {
            setStatus('error_loading_data', { message: err.message });
        });
    }

    function renderDetections(detections) {
        Object.keys(detectionMarkers).forEach(function (key) {
            map.removeLayer(detectionMarkers[key]);
        });
        detectionMarkers = {};

        detections.forEach(function (det) {
            var cx = det.crown_center_x;
            var cy = det.crown_center_y;
            if (cx === null || cy === null) return;

            var options = markerOptions(det, false);

            var circle = L.circle([cy, cx], {
                radius: options.radius,
                color: options.style.color,
                fillColor: options.style.fillColor,
                fillOpacity: options.style.fillOpacity,
                opacity: options.style.opacity,
                weight: options.style.weight,
                dashArray: options.style.dashArray
            }).addTo(map);

            circle.bindTooltip('#' + det.id + ' ' + (det.confidence * 100).toFixed(0) + '%', {
                permanent: true,
                direction: 'top',
                className: 'detection-tooltip'
            });

            circle.on('click', function () {
                if (selectionMode && det.review_status === 'pending') {
                    toggleDetectionSelection(det.id, det);
                    return;
                }
                selectDetection(det, true);
            });

            detectionMarkers[det.id] = circle;
        });
        updateBulkControls();
    }

    function selectDetection(det, panToMarker) {
        selectedDetection = det;

        Object.keys(detectionMarkers).forEach(function (key) {
            var markerDet = findDetectionById(key);
            if (markerDet) applyMarkerStyle(markerDet, String(det.id) === String(key));
        });

        if (detectionMarkers[det.id] && panToMarker) {
            map.panTo(detectionMarkers[det.id].getLatLng());
        }

        document.getElementById('detection-detail').style.display = 'block';
        document.getElementById('det-id').textContent = det.id;
        document.getElementById('det-conf').textContent = (det.confidence * 100).toFixed(1) + '%';
        document.getElementById('det-pos').textContent =
            '(' + (det.crown_center_x || 0).toFixed(0) + ', ' + (det.crown_center_y || 0).toFixed(0) + ')';
        document.getElementById('det-status').textContent = det.review_status;

        var actionsDiv = document.getElementById('det-actions-btns');
        actionsDiv.innerHTML = '';

        if (det.review_status === 'pending') {
            var selectBtn = document.createElement('button');
            selectBtn.className = selectedDetectionIds.has(String(det.id)) ? 'btn btn-outline small' : 'btn small';
            selectBtn.textContent = selectedDetectionIds.has(String(det.id))
                ? t('unselect_detection')
                : t('select_detection');
            selectBtn.onclick = function () { toggleDetectionSelection(det.id, det); };
            actionsDiv.appendChild(selectBtn);

            var confirmBtn = document.createElement('button');
            confirmBtn.className = 'btn success small';
            confirmBtn.textContent = t('confirm');
            confirmBtn.onclick = function () { confirmDetection(det.id); };
            actionsDiv.appendChild(confirmBtn);

            var rejectBtn = document.createElement('button');
            rejectBtn.className = 'btn danger small';
            rejectBtn.textContent = t('reject');
            rejectBtn.onclick = function () { rejectDetection(det.id); };
            actionsDiv.appendChild(rejectBtn);
        } else if (det.review_status === 'confirmed') {
            var linkBtn = document.createElement('a');
            linkBtn.className = 'btn small';
            linkBtn.style.textDecoration = 'none';
            linkBtn.style.display = 'inline-block';
            linkBtn.style.color = 'white';
            linkBtn.textContent = t('view_profile');
            if (det.tree_code) {
                linkBtn.href = 'tree_profile.html?code=' + encodeURIComponent(det.tree_code);
            } else {
                linkBtn.href = '#';
                linkBtn.onclick = function () {
                    alert(t('tree_profile_not_linked'));
                    return false;
                };
            }
            actionsDiv.appendChild(linkBtn);
        }
    }

    function confirmDetection(detId) {
        apiPost('/detections/' + detId + '/confirm').then(function (data) {
            if (data.status === 'ok' && data.tree_code) {
                selectedDetectionIds.delete(String(detId));
                setStatus('confirmed_tree', { code: data.tree_code });
                loadDetections();
                document.getElementById('detection-detail').style.display = 'none';
                selectedDetection = null;
            } else {
                setStatus('confirm_failed', { message: unknownError(data.message) });
            }
        }).catch(function (err) {
            setStatus('confirm_failed', { message: err.message });
        });
    }

    function rejectDetection(detId) {
        apiPost('/detections/' + detId + '/reject').then(function (data) {
            if (data.status === 'ok') {
                selectedDetectionIds.delete(String(detId));
                setStatus('detection_rejected');
                loadDetections();
                document.getElementById('detection-detail').style.display = 'none';
                selectedDetection = null;
            } else {
                setStatus('reject_failed', { message: unknownError(data.message) });
            }
        }).catch(function (err) {
            setStatus('reject_failed', { message: err.message });
        });
    }

    function selectAllPending() {
        pendingDetections().forEach(function (det) {
            selectedDetectionIds.add(String(det.id));
        });
        renderDetections(detectionsCache);
        if (selectedDetection) selectDetection(selectedDetection, false);
        updateBulkControls();
    }

    function clearSelection() {
        selectedDetectionIds.clear();
        renderDetections(detectionsCache);
        if (selectedDetection) selectDetection(selectedDetection, false);
        updateBulkControls();
    }

    function confirmSelectedDetections() {
        var ids = Array.from(selectedDetectionIds);
        if (!ids.length || bulkConfirming) {
            setStatus('no_selected_detections');
            return;
        }
        bulkConfirming = true;
        updateBulkControls();
        setStatus('bulk_confirming', { count: ids.length });

        var confirmed = 0;
        var failed = 0;
        ids.reduce(function (chain, id) {
            return chain.then(function () {
                return apiPost('/detections/' + id + '/confirm').then(function (data) {
                    if (data.status === 'ok') {
                        confirmed += 1;
                        selectedDetectionIds.delete(String(id));
                    } else {
                        failed += 1;
                    }
                }).catch(function () {
                    failed += 1;
                });
            });
        }, Promise.resolve()).then(function () {
            bulkConfirming = false;
            if (failed > 0) {
                setStatus('bulk_confirm_partial', { confirmed: confirmed, failed: failed });
            } else {
                setStatus('bulk_confirmed', { count: confirmed });
            }
            loadDetections();
            document.getElementById('detection-detail').style.display = 'none';
            selectedDetection = null;
        });
    }

    function handleManualPlace(cx, cy) {
        disableManualMode();
        setStatus('placing_tree', { x: cx.toFixed(0), y: cy.toFixed(0) });

        apiPost('/orthomosaics/' + orthoId + '/detections/manual', {
            crown_center_x: cx,
            crown_center_y: cy,
            crown_width: 40,
            crown_height: 40
        }).then(function (data) {
            if (data.status === 'ok') {
                setStatus('manual_tree_added', { id: data.detection_id });
                loadDetections();
            } else {
                setStatus('manual_add_failed', { message: unknownError(data.message) });
            }
        }).catch(function (err) {
            setStatus('manual_add_failed', { message: err.message });
        });
    }

    function enableManualMode() {
        manualMode = true;
        document.getElementById('map').style.cursor = 'crosshair';
        document.getElementById('manual-hint').style.display = 'block';
        document.getElementById('btn-manual-mode').classList.add('btn-danger');
        updateManualButton();
    }

    function disableManualMode() {
        manualMode = false;
        document.getElementById('map').style.cursor = '';
        document.getElementById('manual-hint').style.display = 'none';
        document.getElementById('btn-manual-mode').classList.remove('btn-danger');
        updateManualButton();
    }

    if (!orthoId) {
        document.getElementById('map').innerHTML = '<div class="error-box">' + t('missing_ortho_id') + '</div>';
        return;
    }

    document.getElementById('btn-manual-mode').addEventListener('click', function () {
        if (manualMode) {
            disableManualMode();
        } else {
            enableManualMode();
        }
    });

    document.getElementById('btn-selection-mode').addEventListener('click', toggleSelectionMode);
    document.getElementById('btn-select-all-pending').addEventListener('click', selectAllPending);
    document.getElementById('btn-clear-selection').addEventListener('click', clearSelection);
    document.getElementById('btn-confirm-selected').addEventListener('click', confirmSelectedDetections);

    document.getElementById('btn-detect-palms').addEventListener('click', function () {
        setStatus('running_detection');
        // Step 1: ensure tiles exist before running detection
        apiPost('/orthomosaics/' + orthoId + '/tiles', {
            tile_size: 1024,
            tile_overlap: 0.15
        }).then(function () {
            // Step 2: run real or mock detection over tiles
            return apiPost('/orthomosaics/' + orthoId + '/detect-palms');
        }).then(function (data) {
            if (data.status === 'ok') {
                setStatus('detections_created_from_tiles', {
                    count: data.detections_created,
                    tiles: data.tiles_processed
                });
                loadDetections();
            } else {
                setStatus('detection_failed', { message: unknownError(data.message) });
            }
        }).catch(function (err) {
            setStatus('detection_failed', { message: err.message });
        });
    });

    document.getElementById('btn-refresh').addEventListener('click', function () {
        loadDetections();
    });

    document.addEventListener('op:i18n-change', function () {
        setStatus(lastStatus.key, lastStatus.params);
        updateManualButton();
        updateBulkControls();
        if (selectedDetection) selectDetection(selectedDetection, false);
    });

    updateManualButton();
    updateBulkControls();
    loadOrthoInfo();
})();
