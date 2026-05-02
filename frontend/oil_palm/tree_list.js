document.addEventListener('DOMContentLoaded', () => {
    const plantationSelect = document.getElementById('plantation-select');
    const missionSelect = document.getElementById('mission-select');
    const missionFilterGroup = document.getElementById('mission-filter-group');
    const tableContainer = document.getElementById('table-container');
    const totalBadge = document.getElementById('total-badge');
    const pageInfo = document.getElementById('page-info');
    const btnPrev = document.getElementById('btn-prev');
    const btnNext = document.getElementById('btn-next');
    const pagination = document.getElementById('pagination');

    let currentPage = 1;
    const limit = 15;
    let currentPlantationId = 0;
    let currentMissionId = 0;

    // 1. 加载地块列表
    async function loadPlantations() {
        try {
            const res = await fetch('/api/v1/plantations');
            const data = await res.json();
            const plantations = data.plantations || [];
            
            plantationSelect.innerHTML = '<option value="0">-- All Plantations --</option>';
            plantations.forEach(p => {
                const opt = document.createElement('option');
                opt.value = p.id;
                opt.textContent = `${p.name} (ID: ${p.id}) - ${p.crop_type}`;
                plantationSelect.appendChild(opt);
            });

            // 默认加载全部
            loadTrees();
        } catch (e) {
            console.error('Failed to load plantations', e);
        }
    }

    // 2. 加载选定地块的任务列表
    async function loadMissions(pid) {
        if (pid <= 0) {
            missionFilterGroup.style.display = 'none';
            currentMissionId = 0;
            return;
        }

        try {
            const res = await fetch(`/api/v1/uav/missions?plantation_id=${pid}`);
            const data = await res.json();
            const missions = data.missions || [];

            missionSelect.innerHTML = '<option value="0">-- All Missions --</option>';
            missions.forEach(m => {
                const opt = document.createElement('option');
                opt.value = m.id;
                opt.textContent = `${m.mission_name} (ID: ${m.id})`;
                missionSelect.appendChild(opt);
            });
            
            missionFilterGroup.style.display = 'flex';
        } catch (e) {
            console.error('Failed to load missions', e);
        }
    }

    async function loadTrees() {
        tableContainer.innerHTML = '<div class="empty-state">Loading trees...</div>';
        try {
            let url = `/api/v1/trees?plantation_id=${currentPlantationId}&mission_id=${currentMissionId}&page=${currentPage}&limit=${limit}`;
            const res = await fetch(url);
            const data = await res.json();
            
            const trees = data.trees || [];
            const total = data.total || 0;
            
            totalBadge.textContent = `Total Assets: ${total}`;
            
            if (trees.length === 0) {
                tableContainer.innerHTML = '<div class="empty-state">No trees found in this selection</div>';
                pagination.style.display = 'none';
            } else {
                renderTable(trees);
                updatePagination(total);
            }
        } catch (e) {
            tableContainer.innerHTML = `<div class="empty-state" style="color:var(--danger);">Error: ${e.message}</div>`;
        }
    }

    function renderTable(trees) {
        let html = `<table class="tree-table">
            <thead><tr>
                <th>Code</th><th>Mission</th><th>Species</th><th>Status</th><th>Coordinate</th><th>Action</th>
            </tr></thead><tbody>`;
        trees.forEach(t => {
            const statusClass = `badge-${t.current_status}`;
            const coord = (t.coordinate_x != null && t.coordinate_y != null)
                ? `(${t.coordinate_x.toFixed(1)}, ${t.coordinate_y.toFixed(1)})`
                : '-';
            const mission = t.mission_name || '-';
            html += `<tr>
                <td><a href="tree_profile.html?code=${t.tree_code}">${t.tree_code}</a></td>
                <td>${mission}</td>
                <td>${t.species}</td>
                <td><span class="badge ${statusClass}">${t.current_status}</span></td>
                <td>${coord}</td>
                <td><a href="tree_profile.html?code=${t.tree_code}">View</a></td>
            </tr>`;
        });
        html += '</tbody></table>';
        tableContainer.innerHTML = html;
    }

    function updatePagination(total) {
        pagination.style.display = 'flex';
        const totalPages = Math.ceil(total / limit);
        pageInfo.textContent = `Page ${currentPage} of ${totalPages || 1}`;
        btnPrev.disabled = currentPage <= 1;
        btnNext.disabled = currentPage >= totalPages;
    }

    plantationSelect.addEventListener('change', (e) => {
        currentPlantationId = parseInt(e.target.value);
        currentMissionId = 0; // 换地块时重置任务
        currentPage = 1;
        
        // 选中反馈
        if (currentPlantationId > 0) {
            plantationSelect.classList.add('selected');
        } else {
            plantationSelect.classList.remove('selected');
        }

        loadMissions(currentPlantationId);
        loadTrees();
    });

    missionSelect.addEventListener('change', (e) => {
        currentMissionId = parseInt(e.target.value);
        currentPage = 1;
        loadTrees();
    });

    btnPrev.addEventListener('click', () => {
        if (currentPage > 1) {
            currentPage--;
            loadTrees();
        }
    });

    btnNext.addEventListener('click', () => {
        currentPage++;
        loadTrees();
    });

    loadPlantations();
});
