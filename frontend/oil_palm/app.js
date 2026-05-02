document.addEventListener('DOMContentLoaded', () => {
    const btnCreateMission = document.getElementById('btn-create-mission');
    const btnRegisterOrtho = document.getElementById('btn-register-ortho');
    const btnMockDetections = document.getElementById('btn-mock-detections');
    const missionStatus = document.getElementById('mission-status');
    const orthoStatus = document.getElementById('ortho-status');
    const detectionStatus = document.getElementById('detection-status');
    const detectionList = document.getElementById('detection-list');
    const treeList = document.getElementById('tree-list');
    const btnViewOrtho = document.getElementById('btn-view-ortho');

    let missionId = null;
    let orthoId = null;

    // --- 初始化：尝试恢复最近的状态 ---
    async function init() {
        try {
            // 1. 获取所有任务
            const res = await fetch('/api/v1/uav/missions');
            const data = await res.json();
            const missions = data.missions || [];
            
            if (missions.length > 0) {
                // 取最近的一个任务
                const lastMission = missions[missions.length - 1];
                missionId = lastMission.id;
                missionStatus.textContent = `Current Mission: ${lastMission.mission_name} (ID ${missionId})`;
                btnRegisterOrtho.disabled = false;

                // 2. 获取该任务的正射图
                const orthoRes = await fetch(`/api/v1/uav/missions/${missionId}/orthomosaic`);
                if (orthoRes.ok) {
                    const orthoData = await orthoRes.json();
                    if (orthoData && orthoData.id) {
                        orthoId = orthoData.id;
                        orthoStatus.textContent = `Orthomosaic: ${orthoData.image_url} (ID ${orthoId})`;
                        btnMockDetections.disabled = false;
                        btnViewOrtho.style.display = 'block';
                        btnViewOrtho.href = `ortho_viewer.html?ortho_id=${orthoId}`;
                        
                        // 3. 加载已有的检测点
                        await fetchDetections();
                    }
                }
            }
        } catch (e) {
            console.log('Init state recovery skipped or failed', e);
        }
    }

    init();

    btnCreateMission.addEventListener('click', async () => {
        const plantationName = document.getElementById('plantation-name').value || "Default Plantation";
        const missionName = document.getElementById('mission-name').value || "Unnamed Mission";
        
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
            const pid = data.plantation_id;
            missionStatus.textContent = `Mission created: ${missionName} (ID ${missionId}) under Plantation ID ${pid}`;
            btnRegisterOrtho.disabled = false;
            orthoStatus.textContent = "Status: Ready to register";
        } catch (e) {
            missionStatus.textContent = 'Error: ' + e.message;
        }
    });

    btnRegisterOrtho.addEventListener('click', async () => {
        const orthoUrl = document.getElementById('ortho-url').value;
        if (!orthoUrl) {
            alert("Please provide an image URL");
            return;
        }
        try {
            const res = await fetch(`/api/v1/uav/missions/${missionId}/orthomosaic`, { 
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    width: 658,
                    height: 438,
                    resolution: 0.1,
                    image_url: orthoUrl
                })
            });
            const data = await res.json();
            orthoId = data.orthomosaic_id;
            orthoStatus.textContent = `Orthomosaic registered: ID ${orthoId}`;
            btnMockDetections.disabled = false;
            btnViewOrtho.style.display = 'block';
            btnViewOrtho.href = `ortho_viewer.html?ortho_id=${orthoId}`;
        } catch (e) {
            orthoStatus.textContent = 'Error: ' + e.message;
        }
    });

    btnMockDetections.addEventListener('click', async () => {
        try {
            // 先切瓦片
            const tileRes = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/tiles`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tile_size: 256, tile_overlap: 0.1 })
            });
            
            // 再跑 Mock
            const res = await fetch(`/api/v1/uav/orthomosaics/${orthoId}/detections/mock`, { method: 'POST' });
            const data = await res.json();
            detectionStatus.textContent = `${data.detections_created || 0} detections generated.`;
            
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
