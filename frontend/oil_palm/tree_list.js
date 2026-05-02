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

    // 1. 下拉框开关逻辑
    function setupDropdown(dropdown, list) {
        dropdown.addEventListener('click', (e) => {
            e.stopPropagation();
            const isOpen = list.classList.contains('show');
            closeAllDropdowns();
            if (!isOpen) {
                list.classList.add('show');
                dropdown.classList.add('active');
            }
        });
    }

    function closeAllDropdowns() {
        document.querySelectorAll('.options-list').forEach(l => l.classList.remove('show'));
        document.querySelectorAll('.custom-select').forEach(d => d.classList.remove('active'));
    }

    document.addEventListener('click', closeAllDropdowns);

    setupDropdown(plantationDropdown, plantationOptions);
    setupDropdown(missionDropdown, missionOptions);

    // 2. 加载地块列表
    async function loadPlantations() {
        try {
            const res = await fetch('/api/v1/plantations');
            const data = await res.json();
            const plantations = data.plantations || [];
            
            renderPlantationOptions(plantations);
            loadTrees();
        } catch (e) {
            console.error('Failed to load plantations', e);
        }
    }

    function renderPlantationOptions(plantations) {
        plantationOptions.innerHTML = '';
        
        // "All Plantations" Option
        const allOpt = createOptionItem(0, '-- All Plantations --', true);
        plantationOptions.appendChild(allOpt);

        plantations.forEach(p => {
            const label = `${p.name} (ID: ${p.id})`;
            const opt = createOptionItem(p.id, label, false);
            plantationOptions.appendChild(opt);
        });
    }

    function createOptionItem(id, label, isDefault) {
        const div = document.createElement('div');
        div.className = 'option-item' + (isDefault && currentPlantationId === 0 ? ' selected' : '');
        div.textContent = label;
        div.addEventListener('click', () => {
            if (label.includes('Mission')) {
                handleMissionSelect(id, label);
            } else {
                handlePlantationSelect(id, label);
            }
        });
        return div;
    }

    function handlePlantationSelect(id, label) {
        currentPlantationId = id;
        currentMissionId = 0;
        currentPage = 1;

        plantationDropdown.querySelector('.selected-text').textContent = label;
        updateSelectionHighlight(plantationOptions, id);
        
        // 重置 Mission 文本
        missionDropdown.querySelector('.selected-text').textContent = '-- All Missions --';

        if (id > 0) {
            loadMissions(id);
        } else {
            missionFilterGroup.style.display = 'none';
            loadTrees();
        }
    }

    function handleMissionSelect(id, label) {
        currentMissionId = id;
        currentPage = 1;
        missionDropdown.querySelector('.selected-text').textContent = label;
        updateSelectionHighlight(missionOptions, id);
        loadTrees();
    }

    function updateSelectionHighlight(container, id) {
        container.querySelectorAll('.option-item').forEach(item => {
            item.classList.remove('selected');
        });
        // 这里简化处理，根据我们创建 item 时的逻辑来找
        // 实际中建议把 ID 存入 dataset
    }

    // 重新设计 Option 创建函数以支持更稳健的高亮
    function createImprovedOption(id, label, container, type) {
        const div = document.createElement('div');
        div.className = 'option-item';
        div.dataset.id = id;
        div.textContent = label;
        
        div.addEventListener('click', () => {
            if (type === 'plantation') {
                selectPlantation(id, label);
            } else {
                selectMission(id, label);
            }
        });
        return div;
    }

    async function loadMissions(pid) {
        try {
            const res = await fetch(`/api/v1/uav/missions?plantation_id=${pid}`);
            const data = await res.json();
            const missions = data.missions || [];

            missionOptions.innerHTML = '';
            missionOptions.appendChild(createImprovedOption(0, '-- All Missions --', missionOptions, 'mission'));
            
            missions.forEach(m => {
                missionOptions.appendChild(createImprovedOption(m.id, m.mission_name, missionOptions, 'mission'));
            });
            
            missionFilterGroup.style.display = 'flex';
            loadTrees();
        } catch (e) {
            console.error('Failed to load missions', e);
        }
    }

    function selectPlantation(id, label) {
        currentPlantationId = id;
        currentMissionId = 0;
        currentPage = 1;
        plantationDropdown.querySelector('.selected-text').textContent = label;
        highlightSelected(plantationOptions, id);
        
        if (id > 0) {
            loadMissions(id);
        } else {
            missionFilterGroup.style.display = 'none';
            loadTrees();
        }
    }

    function selectMission(id, label) {
        currentMissionId = id;
        currentPage = 1;
        missionDropdown.querySelector('.selected-text').textContent = label;
        highlightSelected(missionOptions, id);
        loadTrees();
    }

    function highlightSelected(container, id) {
        container.querySelectorAll('.option-item').forEach(item => {
            if (parseInt(item.dataset.id) === id) {
                item.classList.add('selected');
            } else {
                item.classList.remove('selected');
            }
        });
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

    // 初始加载
    async function init() {
        try {
            const res = await fetch('/api/v1/plantations');
            const data = await res.json();
            const plantations = data.plantations || [];
            
            plantationOptions.innerHTML = '';
            plantationOptions.appendChild(createImprovedOption(0, '-- All Plantations --', plantationOptions, 'plantation'));
            plantations.forEach(p => {
                plantationOptions.appendChild(createImprovedOption(p.id, `${p.name} (ID: ${p.id})`, plantationOptions, 'plantation'));
            });
            
            highlightSelected(plantationOptions, 0);
            loadTrees();
        } catch (e) {
            console.error('Init failed', e);
        }
    }

    init();
});
