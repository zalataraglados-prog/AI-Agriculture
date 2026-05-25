document.addEventListener('DOMContentLoaded', async () => {
    const dropdown = document.getElementById('plantation-dropdown');
    const options = document.getElementById('plantation-options');
    const params = new URLSearchParams(window.location.search);
    const preferredId = params.get('plantation_id');
    const state = {
        plantations: [],
        currentPlantationId: preferredId ? Number(preferredId) : null,
        stats: {},
        priority: [],
        blocks: []
    };

    function t(key, params = {}) {
        if (window.OP_I18N) return window.OP_I18N.t(key, params);
        return Object.entries(params).reduce((out, [name, value]) => out.replaceAll(`{${name}}`, value), key);
    }

    function closeAll() {
        options.classList.remove('show');
        dropdown.classList.remove('active');
        options.closest('.filter-group')?.classList.remove('dropdown-open');
    }

    document.addEventListener('click', closeAll);

    dropdown.onclick = (e) => {
        e.stopPropagation();
        const wasOpen = options.classList.contains('show');
        closeAll();
        if (!wasOpen) {
            options.classList.add('show');
            dropdown.classList.add('active');
            options.closest('.filter-group')?.classList.add('dropdown-open');
        }
    };

    async function selectPlantation(plantation, pushUrl = true) {
        state.currentPlantationId = Number(plantation.id);
        document.getElementById('plantation-text').textContent = plantationLabel(plantation);
        closeAll();
        renderPlantationOptions();
        await loadDashboard(plantation.id);

        if (pushUrl) {
            const url = new URL(window.location);
            url.searchParams.set('plantation_id', plantation.id);
            window.history.pushState({}, '', url);
        }
    }

    function plantationLabel(plantation) {
        return `${plantation.name} (#${plantation.id})`;
    }

    function renderPlantationOptions() {
        options.innerHTML = '';
        state.plantations.forEach((plantation) => {
            const div = document.createElement('div');
            div.className = 'option-item';
            div.textContent = plantationLabel(plantation);
            if (Number(plantation.id) === Number(state.currentPlantationId)) {
                div.classList.add('selected');
            }
            div.onclick = (e) => {
                e.stopPropagation();
                selectPlantation(plantation);
            };
            options.appendChild(div);
        });
    }

    async function loadPlantations() {
        const text = document.getElementById('plantation-text');

        try {
            const res = await fetch('/api/v1/plantations');
            const data = await res.json();
            state.plantations = data.plantations || [];
            renderPlantationOptions();

            if (!state.plantations.length) {
                text.textContent = t('no_plantations');
                renderStats({});
                renderPriority([]);
                renderBlocks([]);
                return;
            }

            const selected = state.plantations.find((p) => Number(p.id) === Number(state.currentPlantationId)) || state.plantations[0];
            await selectPlantation(selected, false);
        } catch (e) {
            console.error('Failed to load plantations', e);
            text.textContent = t('error_loading_plantations');
        }
    }

    async function loadDashboard(plantationId) {
        if (!plantationId) return;
        try {
            const [dashboardRes, blocksRes] = await Promise.all([
                fetch(`/api/v1/plantations/${plantationId}/dashboard`),
                fetch(`/api/v1/plantations/${plantationId}/blocks/report`)
            ]);
            const dashboardData = await dashboardRes.json();
            const blocksData = await blocksRes.json();

            const dashboard = dashboardData.dashboard || {};
            const report = blocksData.report || {};

            state.stats = dashboard.stats || {};
            state.priority = dashboard.priority_trees || [];
            state.blocks = report.blocks || [];

            renderStats(state.stats);
            renderPriority(state.priority);
            renderBlocks(state.blocks);
        } catch (e) {
            console.error('Failed to load dashboard', e);
            renderStats({});
            renderPriority([]);
            renderBlocks([]);
        }
    }

    function renderStats(stats) {
        const grid = document.getElementById('stats-grid');
        const cards = [
            ['stat_total_trees', stats.total_trees],
            ['stat_active', stats.active_trees],
            ['stat_complete', stats.complete_assessments],
            ['stat_harvest', stats.harvest_recommended],
            ['stat_disease_risk', stats.disease_risk],
            ['stat_missing_evidence', stats.missing_evidence]
        ];
        grid.innerHTML = cards.map(([labelKey, value]) => `
            <div class="stat-card">
                <span>${t(labelKey)}</span>
                <strong>${value ?? 0}</strong>
            </div>
        `).join('');
    }

    function renderPriority(items) {
        const el = document.getElementById('priority-list');
        if (!items || !items.length) {
            el.textContent = t('no_priority_trees');
            return;
        }
        el.innerHTML = items.map((item) => `
            <div class="tree-item">
                <div>
                    <strong>${item.tree_code}</strong>
                    <div style="color:#94a3b8;font-size:0.82rem;">${item.summary || '-'}</div>
                </div>
                <a class="btn small" href="tree_profile.html?code=${encodeURIComponent(item.tree_code)}">${item.recommended_action || t('view_profile')}</a>
            </div>
        `).join('');
    }

    function renderBlocks(blocks) {
        const el = document.getElementById('block-report');
        if (!blocks || !blocks.length) {
            el.innerHTML = `<div class="status-box">${t('no_block_data')}</div>`;
            return;
        }
        el.innerHTML = `
            <table class="report-table">
                <thead>
                    <tr>
                        <th>${t('block')}</th>
                        <th>${t('trees')}</th>
                        <th>${t('harvest')}</th>
                        <th>${t('disease_risk')}</th>
                        <th>${t('missing_evidence')}</th>
                    </tr>
                </thead>
                <tbody>
                    ${blocks.map((block) => `
                        <tr>
                            <td>${block.block_id}</td>
                            <td>${block.total_trees}</td>
                            <td>${block.harvest_recommended}</td>
                            <td>${block.disease_risk}</td>
                            <td>${block.missing_evidence}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    }

    document.addEventListener('op:i18n-change', () => {
        renderPlantationOptions();
        const selected = state.plantations.find((p) => Number(p.id) === Number(state.currentPlantationId));
        if (selected) document.getElementById('plantation-text').textContent = plantationLabel(selected);
        renderStats(state.stats);
        renderPriority(state.priority);
        renderBlocks(state.blocks);
    });

    await loadPlantations();
});
