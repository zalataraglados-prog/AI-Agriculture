document.addEventListener('DOMContentLoaded', async () => {
    const dropdown = document.getElementById('plantation-dropdown');
    const options = document.getElementById('plantation-options');
    const params = new URLSearchParams(window.location.search);
    const preferredId = params.get('plantation_id');

    // --- Dropdown Interaction ---
    function closeAll() {
        options.classList.remove('show');
        dropdown.classList.remove('active');
    }

    document.addEventListener('click', closeAll);

    dropdown.onclick = (e) => {
        e.stopPropagation();
        const wasOpen = options.classList.contains('show');
        closeAll();
        if (!wasOpen) {
            options.classList.add('show');
            dropdown.classList.add('active');
        }
    };

    await loadPlantations(preferredId);
});

async function loadPlantations(preferredId) {
    const options = document.getElementById('plantation-options');
    const text = document.getElementById('plantation-text');

    try {
        const res = await fetch('/api/v1/plantations');
        const data = await res.json();
        const list = data.plantations || [];

        options.innerHTML = '';
        list.forEach(p => {
            const div = document.createElement('div');
            div.className = 'option-item';
            div.textContent = `${p.name} (#${p.id})`;
            if (String(p.id) === String(preferredId)) {
                div.classList.add('selected');
                text.textContent = div.textContent;
                loadDashboard(p.id);
            }
            div.onclick = (e) => {
                e.stopPropagation();
                document.querySelectorAll('.option-item').forEach(el => el.classList.remove('selected'));
                div.classList.add('selected');
                text.textContent = div.textContent;
                options.classList.remove('show');
                const dropdown = document.getElementById('plantation-dropdown');
                dropdown.classList.remove('active');
                loadDashboard(p.id);
                // Update URL without reload
                const url = new URL(window.location);
                url.searchParams.set('plantation_id', p.id);
                window.history.pushState({}, '', url);
            };
            options.appendChild(div);
        });

        // If no preference but we have data, select first by default
        if (!preferredId && list.length > 0) {
            const first = options.firstChild;
            first.click();
        }
    } catch (e) {
        console.error('Failed to load plantations', e);
        text.textContent = 'Error loading plantations';
    }
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
