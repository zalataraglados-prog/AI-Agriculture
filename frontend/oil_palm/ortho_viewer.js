(function () {
    'use strict';

    var orthoId = getQueryParam('ortho_id');
    if (!orthoId) {
        document.getElementById('map').innerHTML = '<div class="error-box">Missing ?ortho_id= parameter</div>';
        return;
    }

    var map = null;
    var imageOverlay = null;
    var detectionMarkers = {};
    var selectedDetId = null;
    var manualMode = false;
    var orthoWidth = 0;
    var orthoHeight = 0;
    var orthoResolution = 0;

    var STATUS_COLORS = {
        pending: '#f59e0b',
        confirmed: '#10b981',
        rejected: '#ef4444',
        corrected: '#6366f1'
    };

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

    function setStatus(msg) {
        document.getElementById('action-status').textContent = msg;
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
        for (var x = 0; x < w; x += 200) {
            for (var y = 0; y < h; y += 200) {
                ctx.fillText(x + ',' + y, x * scaleX + 2, y * scaleY + gridSize * 0.5);
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

        // 优先使用真实图片，如果没有则使用占位图
        var imgUrl = realImgUrl || generatePlaceholderImage(w, h);
        var bounds = [[0, 0], [h, w]];
        imageOverlay = L.imageOverlay(imgUrl, bounds).addTo(map);
        map.fitBounds(bounds);

        map.on('mousemove', function (e) {
            document.getElementById('coords-display').textContent =
                'X: ' + e.latlng.lng.toFixed(1) + '  Y: ' + e.latlng.lat.toFixed(1);
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
                setStatus('Failed to load orthomosaic info');
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
            setStatus('Error loading ortho: ' + err.message);
        });
    }

    function loadDetections() {
        apiGet('/orthomosaics/' + orthoId + '/detections').then(function (data) {
            if (data.status !== 'ok') {
                setStatus('Failed to load detections');
                return;
            }
            var detections = data.detections || [];
            document.getElementById('info-det-count').textContent = detections.length;
            renderDetections(detections);
            setStatus(detections.length + ' detections loaded');
        }).catch(function (err) {
            setStatus('Error loading detections: ' + err.message);
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

            var color = STATUS_COLORS[det.review_status] || '#94a3b8';
            var opacity = det.review_status === 'confirmed' ? 0.6 : 0.85;

            var circle = L.circle([cy, cx], {
                radius: 15,
                color: color,
                fillColor: color,
                fillOpacity: opacity,
                weight: 2
            }).addTo(map);

            circle.bindTooltip('#' + det.id + ' ' + (det.confidence * 100).toFixed(0) + '%', {
                permanent: true,
                direction: 'top',
                className: 'detection-tooltip'
            });

            circle.on('click', function () {
                selectDetection(det);
            });

            detectionMarkers[det.id] = circle;
        });
    }

    function selectDetection(det) {
        selectedDetId = det.id;

        Object.keys(detectionMarkers).forEach(function (key) {
            detectionMarkers[key].setStyle({ weight: 2 });
        });

        if (detectionMarkers[det.id]) {
            detectionMarkers[det.id].setStyle({ weight: 4, fillOpacity: 1 });
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
            var confirmBtn = document.createElement('button');
            confirmBtn.className = 'btn success small';
            confirmBtn.textContent = 'Confirm';
            confirmBtn.onclick = function () { confirmDetection(det.id); };
            actionsDiv.appendChild(confirmBtn);

            var rejectBtn = document.createElement('button');
            rejectBtn.className = 'btn danger small';
            rejectBtn.textContent = 'Reject';
            rejectBtn.onclick = function () { rejectDetection(det.id); };
            actionsDiv.appendChild(rejectBtn);
        } else if (det.review_status === 'confirmed') {
            var linkBtn = document.createElement('a');
            linkBtn.className = 'btn small';
            linkBtn.style.textDecoration = 'none';
            linkBtn.style.display = 'inline-block';
            linkBtn.style.color = 'white';
            linkBtn.textContent = 'View Tree Profile';
            linkBtn.href = '#';
            linkBtn.onclick = function () {
                apiPost('/detections/' + det.id + '/confirm').then(function (d) {
                    if (d.tree_code) {
                        window.location.href = 'tree_profile.html?code=' + d.tree_code;
                    }
                });
                return false;
            };
            actionsDiv.appendChild(linkBtn);
        }
    }

    function confirmDetection(detId) {
        apiPost('/detections/' + detId + '/confirm').then(function (data) {
            if (data.status === 'ok' && data.tree_code) {
                setStatus('Confirmed! Tree: ' + data.tree_code);
                loadDetections();
                document.getElementById('detection-detail').style.display = 'none';
                selectedDetId = null;
            } else {
                setStatus('Confirm failed: ' + (data.message || 'unknown error'));
            }
        });
    }

    function rejectDetection(detId) {
        apiPost('/detections/' + detId + '/reject').then(function (data) {
            if (data.status === 'ok') {
                setStatus('Detection rejected');
                loadDetections();
                document.getElementById('detection-detail').style.display = 'none';
                selectedDetId = null;
            } else {
                setStatus('Reject failed: ' + (data.message || 'unknown error'));
            }
        });
    }

    function handleManualPlace(cx, cy) {
        disableManualMode();
        setStatus('Placing tree at (' + cx.toFixed(0) + ', ' + cy.toFixed(0) + ')...');

        apiPost('/orthomosaics/' + orthoId + '/detections/manual', {
            crown_center_x: cx,
            crown_center_y: cy,
            crown_width: 40,
            crown_height: 40
        }).then(function (data) {
            if (data.status === 'ok') {
                setStatus('Manual tree added, detection #' + data.detection_id);
                loadDetections();
            } else {
                setStatus('Manual add failed: ' + (data.message || 'unknown error'));
            }
        });
    }

    function enableManualMode() {
        manualMode = true;
        document.getElementById('map').style.cursor = 'crosshair';
        document.getElementById('manual-hint').style.display = 'block';
        document.getElementById('btn-manual-mode').textContent = 'Cancel Manual Mode';
        document.getElementById('btn-manual-mode').classList.add('btn-danger');
    }

    function disableManualMode() {
        manualMode = false;
        document.getElementById('map').style.cursor = '';
        document.getElementById('manual-hint').style.display = 'none';
        document.getElementById('btn-manual-mode').textContent = 'Add Tree Manually';
        document.getElementById('btn-manual-mode').classList.remove('btn-danger');
    }

    document.getElementById('btn-manual-mode').addEventListener('click', function () {
        if (manualMode) {
            disableManualMode();
        } else {
            enableManualMode();
        }
    });

    document.getElementById('btn-detect-palms').addEventListener('click', function () {
        setStatus('Running mock detection...');
        apiPost('/orthomosaics/' + orthoId + '/detect-palms').then(function (data) {
            if (data.status === 'ok') {
                setStatus(data.detections_created + ' detections created from ' + data.tiles_processed + ' tiles');
                loadDetections();
            } else {
                setStatus('Detection failed: ' + (data.message || 'unknown error'));
            }
        });
    });

    document.getElementById('btn-refresh').addEventListener('click', function () {
        loadDetections();
    });

    loadOrthoInfo();
})();
