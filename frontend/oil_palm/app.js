document.addEventListener('DOMContentLoaded', () => {
    const btnCreateMission = document.getElementById('btn-create-mission');
    const btnRegisterOrtho = document.getElementById('btn-register-ortho');
    const btnMockDetections = document.getElementById('btn-mock-detections');
    const missionStatus = document.getElementById('mission-status');
    const orthoStatus = document.getElementById('ortho-status');
    const detectionStatus = document.getElementById('detection-status');
    const detectionList = document.getElementById('detection-list');
    const treeList = document.getElementById('tree-list');

    let missionId = null;
    let orthoId = null;

    btnCreateMission.addEventListener('click', async () => {
        try {
            const res = await fetch('/api/v1/uav/missions', { method: 'POST' });
            const data = await res.json();
            missionId = data.mission_id || 1;
            missionStatus.textContent = `Mission created: ID ${missionId}`;
            btnRegisterOrtho.disabled = false;
        } catch (e) {
            missionStatus.textContent = 'Error: ' + e.message;
        }
    });

    btnRegisterOrtho.addEventListener('click', async () => {
        try {
            const res = await fetch(`/api/v1/uav/missions/${missionId}/orthomosaic`, { method: 'POST' });
            const data = await res.json();
            orthoId = data.orthomosaic_id || 1;
            orthoStatus.textContent = `Orthomosaic registered: ID ${orthoId}`;
            btnMockDetections.disabled = false;
        } catch (e) {
            orthoStatus.textContent = 'Error: ' + e.message;
        }
    });

    btnMockDetections.addEventListener('click', async () => {
        try {
            await fetch(`/api/v1/uav/orthomosaics/${orthoId}/tiles`, { method: 'POST' });
            const res = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/detections/mock`, { method: 'POST' });
            const data = await res.json();
            detectionStatus.textContent = `${data.detections_created || 3} mock detections generated.`;
            renderMockDetections();
        } catch (e) {
            detectionStatus.textContent = 'Error: ' + e.message;
        }
    });

    function renderMockDetections() {
        detectionList.innerHTML = '';
        for (let i = 1; i <= 3; i++) {
            const div = document.createElement('div');
            div.className = 'detection-item';
            div.innerHTML = `
                <span>Detection #${i} (Conf: 0.9${i})</span>
                <div class="detection-actions">
                    <button class="btn success" onclick="confirmDetection(${i}, this.parentElement.parentElement)">Confirm</button>
                    <button class="btn danger" onclick="rejectDetection(${i}, this.parentElement.parentElement)">Reject</button>
                </div>
            `;
            detectionList.appendChild(div);
        }
    }

    window.confirmDetection = async (id, element) => {
        try {
            const res = await fetch(`/api/v1/uav/detections/${id}/confirm`, { method: 'POST' });
            const data = await res.json();
            element.remove();
            
            const treeDiv = document.createElement('div');
            treeDiv.className = 'tree-item';
            treeDiv.innerHTML = `<span>🌳 Tree: <strong>${data.tree_code || 'OP-000001'}</strong></span>`;
            treeList.appendChild(treeDiv);
        } catch (e) {
            alert('Confirm failed');
        }
    };

    window.rejectDetection = async (id, element) => {
        try {
            await fetch(`/api/v1/uav/detections/${id}/reject`, { method: 'POST' });
            element.remove();
        } catch (e) {
            alert('Reject failed');
        }
    };
});
