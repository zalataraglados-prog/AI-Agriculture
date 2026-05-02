document.addEventListener('DOMContentLoaded', () => {
    // Dropdown Elements
    const plantationDropdown = document.getElementById('plantation-dropdown');
    const plantationOptions = document.getElementById('plantation-options');
    const missionDropdown = document.getElementById('mission-dropdown');
    const missionOptions = document.getElementById('mission-options');
    const missionFilterGroup = document.getElementById('mission-filter-group');

    // UI Elements
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

    // 1. 下拉框全局管理
    function closeAllDropdowns() {
        document.querySelectorAll('.options-list').forEach(l => l.classList.remove('show'));
        document.querySelectorAll('.custom-select').forEach(d => d.classList.remove('active'));
    }

    document.addEventListener('click', closeAllDropdowns);

    plantationDropdown.addEventListener('click', (e) => {
        e.stopPropagation();
        const isOpen = plantationOptions.classList.contains('show');
        closeAllDropdowns();
        if (!isOpen) {
            plantationOptions.classList.add('show');
            plantationDropdown.classList.add('active');
        }
    });

    missionDropdown.addEventListener('click', (e) => {
        e.stopPropagation();
        const isOpen = missionOptions.classList.contains('show');
        closeAllDropdowns();
        if (!isOpen) {
            missionOptions.classList.add('show');
            missionDropdown.classList.add('active');
        }
    });

    // 2. 创建选项项
    function createOption(id, label, type) {
        const div = document.createElement('div');
        div.className = 'option-item';
        div.dataset.id = id;
        div.textContent = label;
        
        div.addEventListener('click', (e) => {
            e.stopPropagation();
            if (type === 'plantation') {
                handlePlantationSelect(id, label);
            } else {
                handleMissionSelect(id, label);
            }
            closeAllDropdowns();
        });
        return div;
    }

    async function handlePlantationSelect(id, label) {
        currentPlantationId = id;
        currentMissionId = 0;
        currentPage = 1;

        plantationDropdown.querySelector('.selected-text').textContent = label;
        highlightItem(plantationOptions, id);
        
        // 重置 Mission 下拉框
        missionDropdown.querySelector('.selected-text').textContent = '-- All Missions --';

        if (id > 0) {
            await loadMissions(id);
        } else {
            missionFilterGroup.style.display = 'none';
        }
        loadTrees();
    }

    async function handleMissionSelect(id, label) {
        currentMissionId = id;
        currentPage = 1;
        missionDropdown.querySelector('.selected-text').textContent = label;
        highlightItem(missionOptions, id);
        loadTrees();
    }

    function highlightItem(container, id) {
        container.querySelectorAll('.option-item').forEach(item => {
            if (parseInt(item.dataset.id) === id) {
                item.classList.add('selected');
            } else {
                item.classList.remove('selected');
            }
        });
    }

    // 3. 数据加载逻辑
    async function loadPlantations() {
        try {
            const res = await fetch('/api/v1/plantations');
            const data = await res.json();
            const list = data.plantations || [];
            
            plantationOptions.innerHTML = '';
            plantationOptions.appendChild(createOption(0, '-- All Plantations --', 'plantation'));
            list.forEach(p => {
                plantationOptions.appendChild(createOption(p.id, `${p.name} (ID: ${p.id})`, 'plantation'));
            });
            
            highlightItem(plantationOptions, 0);
        } catch (e) {
            console.error('Load plantations failed', e);
        }
    }

    async function loadMissions(pid) {
        try {
            const res = await fetch(`/api/v1/uav/missions?plantation_id=${pid}`);
            const data = await res.json();
            const list = data.missions || [];

            missionOptions.innerHTML = '';
            missionOptions.appendChild(createOption(0, '-- All Missions --', 'mission'));
            list.forEach(m => {
                missionOptions.appendChild(createOption(m.id, m.mission_name, 'mission'));
            });
            
            highlightItem(missionOptions, 0);
            missionFilterGroup.style.display = 'flex';
        } catch (e) {
            console.error('Load missions failed', e);
        }
    }

    async function loadTrees() {
        tableContainer.innerHTML = '<div class="empty-state">Syncing data...</div>';
        try {
            const url = `/api/v1/trees?plantation_id=${currentPlantationId}&mission_id=${currentMissionId}&page=${currentPage}&limit=${limit}`;
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
            html += `<tr>
                <td><a href="tree_profile.html?code=${t.tree_code}">${t.tree_code}</a></td>
                <td>${t.mission_name || '-'}</td>
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

    btnPrev.addEventListener('click', () => {
        if (currentPage > 1) { currentPage--; loadTrees(); }
    });

    btnNext.addEventListener('click', () => {
        currentPage++; loadTrees();
    });

    // 4. 初始化
    loadPlantations();
    loadTrees();
});
