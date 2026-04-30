document.addEventListener('DOMContentLoaded', async () => {
    const select = document.getElementById('plantation-select');
    const tableContainer = document.getElementById('table-container');
    const totalBadge = document.getElementById('total-badge');
    const pagination = document.getElementById('pagination');
    const btnPrev = document.getElementById('btn-prev');
    const btnNext = document.getElementById('btn-next');
    const pageInfo = document.getElementById('page-info');

    let currentPage = 1;
    const limit = 20;
    let totalTrees = 0;

    // Load plantations
    try {
        const res = await fetch('/api/v1/plantations');
        const data = await res.json();
        const plantations = data.plantations || [];
        select.innerHTML = '<option value="">-- Select --</option>';
        plantations.forEach(p => {
            const opt = document.createElement('option');
            opt.value = p.id;
            opt.textContent = `${p.name} (${p.crop_type})`;
            select.appendChild(opt);
        });
        // Auto-select first if only one
        if (plantations.length === 1) {
            select.value = plantations[0].id;
            loadTrees();
        }
    } catch (e) {
        select.innerHTML = '<option value="">Failed to load</option>';
    }

    select.addEventListener('change', () => {
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
        const totalPages = Math.ceil(totalTrees / limit);
        if (currentPage < totalPages) {
            currentPage++;
            loadTrees();
        }
    });

    async function loadTrees() {
        const pid = select.value;
        if (!pid) {
            tableContainer.innerHTML = '<div class="empty-state">Select a plantation to view trees</div>';
            pagination.style.display = 'none';
            totalBadge.textContent = '';
            return;
        }

        try {
            const res = await fetch(`/api/v1/trees?plantation_id=${pid}&page=${currentPage}&limit=${limit}`);
            const data = await res.json();
            const trees = data.trees || [];
            totalTrees = data.total || 0;

            totalBadge.textContent = `Total: ${totalTrees} trees`;

            if (trees.length === 0) {
                tableContainer.innerHTML = '<div class="empty-state">No trees registered in this plantation</div>';
                pagination.style.display = 'none';
                return;
            }

            const totalPages = Math.ceil(totalTrees / limit);
            renderTable(trees);
            updatePagination(totalPages);
        } catch (e) {
            tableContainer.innerHTML = `<div class="empty-state">Error: ${e.message}</div>`;
        }
    }

    function renderTable(trees) {
        let html = `<table class="tree-table">
            <thead><tr>
                <th>Code</th><th>Species</th><th>Status</th><th>Coordinate</th><th>Action</th>
            </tr></thead><tbody>`;
        trees.forEach(t => {
            const statusClass = `badge-${t.current_status}`;
            const coord = (t.coordinate_x != null && t.coordinate_y != null)
                ? `(${t.coordinate_x.toFixed(1)}, ${t.coordinate_y.toFixed(1)})`
                : '-';
            html += `<tr>
                <td><a href="tree_profile.html?code=${t.tree_code}">${t.tree_code}</a></td>
                <td>${t.species}</td>
                <td><span class="badge ${statusClass}">${t.current_status}</span></td>
                <td>${coord}</td>
                <td><a href="tree_profile.html?code=${t.tree_code}">View</a></td>
            </tr>`;
        });
        html += '</tbody></table>';
        tableContainer.innerHTML = html;
    }

    function updatePagination(totalPages) {
        if (totalPages <= 1) {
            pagination.style.display = 'none';
            return;
        }
        pagination.style.display = 'flex';
        pageInfo.textContent = `Page ${currentPage} of ${totalPages}`;
        btnPrev.disabled = currentPage <= 1;
        btnNext.disabled = currentPage >= totalPages;
    }
});
