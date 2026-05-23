(function () {
    'use strict';

    function t(key, params = {}) {
        if (window.OP_I18N) return window.OP_I18N.t(key, params);
        return Object.entries(params).reduce((out, [name, value]) => out.replaceAll(`{${name}}`, value), key);
    }

    function init() {
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

        for (const key in els) {
            if (!els[key]) {
                console.error(`Missing element: ${key}`);
                return;
            }
        }

        const state = {
            currentPage: 1,
            limit: 15,
            plantationId: 0,
            missionId: 0,
            plantations: [],
            missions: [],
            lastTrees: [],
            lastTotal: 0
        };

        function closeAll() {
            els.pOptions.classList.remove('show');
            els.mOptions.classList.remove('show');
            els.pDropdown.classList.remove('active');
            els.mDropdown.classList.remove('active');
        }

        function setMissionEnabled(enabled) {
            els.mDropdown.style.opacity = enabled ? '1' : '0.5';
            els.mDropdown.style.pointerEvents = enabled ? 'auto' : 'none';
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

        function plantationLabel(plantation) {
            if (!plantation) return t('all_plantations');
            return `${plantation.name} (ID: ${plantation.id})`;
        }

        function missionLabel(mission) {
            if (!mission) return t('all_missions');
            const dateStr = mission.created_at ? mission.created_at.substring(5, 10) : t('not_available');
            return `${mission.mission_name} (#${mission.id}, ${dateStr})`;
        }

        function selectedPlantation() {
            return state.plantations.find((p) => Number(p.id) === Number(state.plantationId));
        }

        function selectedMission() {
            return state.missions.find((m) => Number(m.id) === Number(state.missionId));
        }

        function updateSelectedText() {
            els.pText.textContent = plantationLabel(selectedPlantation());
            if (state.plantationId > 0) {
                els.mText.textContent = missionLabel(selectedMission());
                setMissionEnabled(true);
            } else {
                els.mText.textContent = t('select_plantation_hint');
                setMissionEnabled(false);
            }
        }

        function createOpt(id, label, type, selected) {
            const div = document.createElement('div');
            div.className = 'option-item' + (selected ? ' selected' : '');
            div.textContent = label;
            div.onclick = async (e) => {
                e.stopPropagation();
                if (type === 'plantation') {
                    await selectPlantation(id);
                } else {
                    await selectMission(id);
                }
                closeAll();
            };
            return div;
        }

        function renderPlantationOptions() {
            els.pOptions.innerHTML = '';
            els.pOptions.appendChild(createOpt(0, t('all_plantations'), 'plantation', state.plantationId === 0));
            state.plantations.forEach((plantation) => {
                els.pOptions.appendChild(createOpt(
                    plantation.id,
                    plantationLabel(plantation),
                    'plantation',
                    Number(plantation.id) === Number(state.plantationId)
                ));
            });
        }

        function renderMissionOptions() {
            els.mOptions.innerHTML = '';
            els.mOptions.appendChild(createOpt(0, t('all_missions'), 'mission', state.missionId === 0));
            state.missions.forEach((mission) => {
                els.mOptions.appendChild(createOpt(
                    mission.id,
                    missionLabel(mission),
                    'mission',
                    Number(mission.id) === Number(state.missionId)
                ));
            });
        }

        async function selectPlantation(id) {
            state.plantationId = Number(id);
            state.missionId = 0;
            state.currentPage = 1;
            state.missions = [];
            updateSelectedText();
            renderPlantationOptions();

            if (state.plantationId > 0) {
                await loadMissions(state.plantationId);
            } else {
                renderMissionOptions();
            }
            await loadTrees();
        }

        async function selectMission(id) {
            state.missionId = Number(id);
            state.currentPage = 1;
            updateSelectedText();
            renderMissionOptions();
            await loadTrees();
        }

        async function loadPlantations() {
            try {
                const res = await fetch('/api/v1/plantations');
                const data = await res.json();
                state.plantations = data.plantations || [];
                renderPlantationOptions();
                updateSelectedText();
            } catch (e) {
                console.error('P-load error', e);
            }
        }

        async function loadMissions(pid) {
            try {
                const res = await fetch(`/api/v1/uav/missions?plantation_id=${pid}`);
                const data = await res.json();
                state.missions = data.missions || [];
                renderMissionOptions();
                updateSelectedText();
            } catch (e) {
                console.error('M-load error', e);
            }
        }

        async function loadTrees() {
            els.table.innerHTML = `<div class="empty-state">${t('loading_registry_data')}</div>`;
            try {
                const url = `/api/v1/trees?plantation_id=${state.plantationId}&mission_id=${state.missionId}&page=${state.currentPage}&limit=${state.limit}`;
                const res = await fetch(url);
                const data = await res.json();
                state.lastTrees = data.trees || [];
                state.lastTotal = data.total || 0;

                els.total.textContent = t('total_assets', { count: state.lastTotal });
                if (!state.lastTrees.length) {
                    els.table.innerHTML = `<div class="empty-state">${t('no_matching_trees')}</div>`;
                    els.pagination.style.display = 'none';
                } else {
                    renderTable(state.lastTrees);
                    updatePagination(state.lastTotal);
                }
            } catch (e) {
                els.table.innerHTML = `<div class="empty-state" style="color:#ef4444;">${t('error_loading_data', { message: e.message })}</div>`;
            }
        }

        function renderTable(trees) {
            const rows = trees.map((tree) => {
                const coord = (tree.coordinate_x != null && tree.coordinate_y != null)
                    ? `(${Number(tree.coordinate_x).toFixed(1)}, ${Number(tree.coordinate_y).toFixed(1)})`
                    : '-';
                return `<tr>
                    <td><a href="tree_profile.html?code=${encodeURIComponent(tree.tree_code)}">${tree.tree_code}</a></td>
                    <td>${tree.mission_name || '-'}</td>
                    <td>${tree.species || '-'}</td>
                    <td><span class="badge badge-${tree.current_status}">${tree.current_status || t('unknown')}</span></td>
                    <td>${coord}</td>
                    <td><a href="tree_profile.html?code=${encodeURIComponent(tree.tree_code)}">${t('view')}</a></td>
                </tr>`;
            }).join('');

            els.table.innerHTML = `<table class="tree-table">
                <thead><tr>
                    <th>${t('code')}</th>
                    <th>${t('mission')}</th>
                    <th>${t('species')}</th>
                    <th>${t('status')}</th>
                    <th>${t('coordinate')}</th>
                    <th>${t('action')}</th>
                </tr></thead>
                <tbody>${rows}</tbody>
            </table>`;
        }

        function updatePagination(total) {
            els.pagination.style.display = 'flex';
            const totalPages = Math.ceil(total / state.limit);
            els.pageInfo.textContent = t('page_info', { page: state.currentPage, total: totalPages || 1 });
            els.btnPrev.disabled = state.currentPage <= 1;
            els.btnNext.disabled = state.currentPage >= totalPages;
        }

        els.btnPrev.onclick = () => {
            if (state.currentPage > 1) {
                state.currentPage -= 1;
                loadTrees();
            }
        };

        els.btnNext.onclick = () => {
            state.currentPage += 1;
            loadTrees();
        };

        document.addEventListener('op:i18n-change', async () => {
            renderPlantationOptions();
            renderMissionOptions();
            updateSelectedText();
            if (state.lastTrees.length) {
                renderTable(state.lastTrees);
                updatePagination(state.lastTotal);
            } else {
                await loadTrees();
            }
        });

        loadPlantations();
        loadTrees();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
