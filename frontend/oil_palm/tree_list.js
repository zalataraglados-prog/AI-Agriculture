(function() {
    console.log('Tree List Script V2 - Loaded');

    function init() {
        // --- Elements ---
        const els = {
            pDropdown: document.getElementById('plantation-dropdown'),
            pOptions: document.getElementById('plantation-options'),
            pText: document.getElementById('plantation-text'),
            mDropdown: document.getElementById('mission-dropdown'),
            mOptions: document.getElementById('mission-options'),
            mText: document.getElementById('mission-text'),
            table: document.getElementById('table-container'),
            total: document.getElementById('total-badge'),
            pageInfo: document.getElementById('btn-page-info'),
            btnPrev: document.getElementById('btn-prev'),
            btnNext: document.getElementById('btn-next'),
            pagination: document.getElementById('pagination')
        };

        // 安全检查：如果有任何元素缺失，报错并停止，防止后续崩溃
        for (let key in els) {
            if (!els[key]) {
                console.error(`Missing element: ${key}`);
                return;
            }
        }

        let state = {
            currentPage: 1,
            limit: 15,
            plantationId: 0,
            missionId: 0
        };

        // --- Dropdown Logic ---
        function closeAll() {
            els.pOptions.classList.remove('show');
            els.mOptions.classList.remove('show');
            els.pDropdown.classList.remove('active');
            els.mDropdown.classList.remove('active');
        }

        document.addEventListener('click', closeAll);

        els.pDropdown.onclick = (e) => {
            e.stopPropagation();
            const wasOpen = els.pOptions.classList.contains('show');
            closeAll();
            if (!wasOpen) {
                els.pOptions.classList.add('show');
                els.pDropdown.classList.add('active');
            }
        };

        els.mDropdown.onclick = (e) => {
            e.stopPropagation();
            const wasOpen = els.mOptions.classList.contains('show');
            closeAll();
            if (!wasOpen) {
                els.mOptions.classList.add('show');
                els.mDropdown.classList.add('active');
            }
        };

        // --- Option Creation ---
        function createOpt(id, label, type) {
            const div = document.createElement('div');
            div.className = 'option-item';
            div.textContent = label;
            div.onclick = async (e) => {
                e.stopPropagation();
                if (type === 'plantation') {
                    await selectPlantation(id, label);
                } else {
                    await selectMission(id, label);
                }
                closeAll();
            };
            return div;
        }

        async function selectPlantation(id, label) {
            state.plantationId = id;
            state.missionId = 0;
            state.currentPage = 1;
            els.pText.textContent = label;
            
            if (id > 0) {
                els.mText.textContent = '-- All Missions --';
                els.mDropdown.style.opacity = '1';
                els.mDropdown.style.pointerEvents = 'auto';
                await loadMissions(id);
            } else {
                els.mText.textContent = 'Please select a plantation...';
                els.mDropdown.style.opacity = '0.5';
                els.mDropdown.style.pointerEvents = 'none';
            }
            loadTrees();
        }

        async function selectMission(id, label) {
            state.missionId = id;
            state.currentPage = 1;
            els.mText.textContent = label;
            loadTrees();
        }

        // --- Data Loading ---
        async function loadPlantations() {
            try {
                const res = await fetch('/api/v1/plantations');
                const data = await res.json();
                const list = data.plantations || [];
                els.pOptions.innerHTML = '';
                els.pOptions.appendChild(createOpt(0, '-- All Plantations --', 'plantation'));
                list.forEach(p => {
                    els.pOptions.appendChild(createOpt(p.id, `${p.name} (ID: ${p.id})`, 'plantation'));
                });
            } catch (e) { console.error('P-load error', e); }
        }

        async function loadMissions(pid) {
            try {
                const res = await fetch(`/api/v1/uav/missions?plantation_id=${pid}`);
                const data = await res.json();
                const list = data.missions || [];
                els.mOptions.innerHTML = '';
                els.mOptions.appendChild(createOpt(0, '-- All Missions --', 'mission'));
                list.forEach(m => {
                    els.mOptions.appendChild(createOpt(m.id, m.mission_name, 'mission'));
                });
            } catch (e) { console.error('M-load error', e); }
        }

        async function loadTrees() {
            els.table.innerHTML = '<div class="empty-state">Loading registry data...</div>';
            try {
                const url = `/api/v1/trees?plantation_id=${state.plantationId}&mission_id=${state.missionId}&page=${state.currentPage}&limit=${state.limit}`;
                const res = await fetch(url);
                const data = await res.json();
                const trees = data.trees || [];
                const total = data.total || 0;
                
                els.total.textContent = `Total Assets: ${total}`;
                if (trees.length === 0) {
                    els.table.innerHTML = '<div class="empty-state">No matching tree records found</div>';
                    els.pagination.style.display = 'none';
                } else {
                    renderTable(trees);
                    updatePagination(total);
                }
            } catch (e) { 
                els.table.innerHTML = '<div class="empty-state" style="color:#ef4444;">Error loading data: ' + e.message + '</div>';
            }
        }

        function renderTable(trees) {
            let html = `<table class="tree-table">
                <thead><tr>
                    <th>Code</th><th>Mission</th><th>Species</th><th>Status</th><th>Coordinate</th><th>Action</th>
                </tr></thead><tbody>`;
            trees.forEach(t => {
                const coord = (t.coordinate_x != null && t.coordinate_y != null)
                    ? `(${t.coordinate_x.toFixed(1)}, ${t.coordinate_y.toFixed(1)})`
                    : '-';
                html += `<tr>
                    <td><a href="tree_profile.html?code=${t.tree_code}">${t.tree_code}</a></td>
                    <td>${t.mission_name || '-'}</td>
                    <td>${t.species}</td>
                    <td><span class="badge badge-${t.current_status}">${t.current_status}</span></td>
                    <td>${coord}</td>
                    <td><a href="tree_profile.html?code=${t.tree_code}">View</a></td>
                </tr>`;
            });
            html += '</tbody></table>';
            els.table.innerHTML = html;
        }

        function updatePagination(total) {
            els.pagination.style.display = 'flex';
            const totalPages = Math.ceil(total / state.limit);
            els.pageInfo.textContent = `Page ${state.currentPage} of ${totalPages || 1}`;
            els.btnPrev.disabled = state.currentPage <= 1;
            els.btnNext.disabled = state.currentPage >= totalPages;
        }

        els.btnPrev.onclick = () => { if (state.currentPage > 1) { state.currentPage--; loadTrees(); } };
        els.btnNext.onclick = () => { state.currentPage++; loadTrees(); };

        // --- Start ---
        loadPlantations();
        loadTrees();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
