document.addEventListener('DOMContentLoaded', async () => {
    const select = document.getElementById('plantation-select');
    const refresh = document.getElementById('btn-refresh');
    const params = new URLSearchParams(window.location.search);

    await loadPlantations(params.get('plantation_id'));
    refresh.addEventListener('click', () => loadDashboard(select.value));
    if (select.value) loadDashboard(select.value);
});

async function loadPlantations(preferredId) {
    const select = document.getElementById('plantation-select');
    const res = await fetch('/api/v1/plantations');
    const data = await res.json();
    const plantations = data.plantations || [];
    select.innerHTML = plantations.map(p => `<option value="${p.id}">${p.name} (#${p.id})</option>`).join('');
    if (preferredId) select.value = preferredId;
}

async function loadDashboard(plantationId) {
    if (!plantationId) return;
    const [dashboardRes, blocksRes] = await Promise.all([
        fetch(`/api/v1/plantations/${plantationId}/dashboard`),
        fetch(`/api/v1/plantations/${plantationId}/blocks/report`)
    ]);
    const dashboard = (await dashboardRes.json()).dashboard;
    const report = (await blocksRes.json()).report;
    renderStats(dashboard.stats || {});
    renderPriority(dashboard.priority_trees || []);
    renderBlocks(report.blocks || []);
}

function renderStats(stats) {
    const grid = document.getElementById('stats-grid');
    const cards = [
        ['Total Trees', stats.total_trees],
        ['Active', stats.active_trees],
        ['Complete', stats.complete_assessments],
        ['Harvest', stats.harvest_recommended],
        ['Disease Risk', stats.disease_risk],
        ['Missing Evidence', stats.missing_evidence]
    ];
    grid.innerHTML = cards.map(([label, value]) => `
        <div class="stat-card">
            <span>${label}</span>
            <strong>${value ?? 0}</strong>
        </div>
    `).join('');
}

function renderPriority(items) {
    const el = document.getElementById('priority-list');
    if (!items.length) {
        el.textContent = 'No priority trees';
        return;
    }
    el.innerHTML = items.map(item => `
        <div class="tree-item">
            <div>
                <strong>${item.tree_code}</strong>
                <div style="color:#94a3b8;font-size:0.82rem;">${item.summary}</div>
            </div>
            <a class="btn small" href="tree_profile.html?code=${item.tree_code}">${item.recommended_action}</a>
        </div>
    `).join('');
}

function renderBlocks(blocks) {
    const el = document.getElementById('block-report');
    if (!blocks.length) {
        el.innerHTML = '<div class="status-box">No block data</div>';
        return;
    }
    el.innerHTML = `
        <table class="report-table">
            <thead>
                <tr>
                    <th>Block</th>
                    <th>Trees</th>
                    <th>Harvest</th>
                    <th>Disease Risk</th>
                    <th>Missing Evidence</th>
                </tr>
            </thead>
            <tbody>
                ${blocks.map(block => `
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
