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
            const res = await fetch('/api/v1/uav/missions', { 
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ plantation_id: 0, mission_name: 'test-mission' })
            });
            const data = await res.json();
            missionId = data.mission_id;
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
            orthoId = data.orthomosaic_id;
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
            
            await fetchDetections();
        } catch (e) {
            detectionStatus.textContent = 'Error: ' + e.message;
        }
    });

    async function fetchDetections() {
        try {
            const res = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/detections`);
            const data = await res.json();
            renderDetections(data.detections || []);
        } catch (e) {
            console.error('fetch detections failed', e);
        }
    }

    function renderDetections(detections) {
        detectionList.innerHTML = '';
        detections.forEach(d => {
            if (d.review_status !== 'pending') return;
            const div = document.createElement('div');
            div.className = 'detection-item';
            div.innerHTML = `
                <span>Detection #${d.id} (Conf: ${d.confidence.toFixed(2)})</span>
                <div class="detection-actions">
                    <button class="btn success" onclick="confirmDetection(${d.id}, this.parentElement.parentElement)">Confirm</button>
                    <button class="btn danger" onclick="rejectDetection(${d.id}, this.parentElement.parentElement)">Reject</button>
                </div>
            `;
            detectionList.appendChild(div);
        });
    }

    window.confirmDetection = async (id, element) => {
        try {
            const res = await fetch(`/api/v1/uav/detections/${id}/confirm`, { method: 'POST' });
            const data = await res.json();
            element.remove();
            
            const treeDiv = document.createElement('div');
            treeDiv.className = 'tree-item';
            treeDiv.innerHTML = `<span>🌳 Tree: <a href="tree_profile.html?code=${data.tree_code}" style="color:#60a5fa;text-decoration:none;font-weight:700;">${data.tree_code}</a></span>`;
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
