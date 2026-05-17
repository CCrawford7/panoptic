// Panoptic Web Dashboard
let projects = [];
let filtered = [];
let currentFilter = 'all';
let currentDetailIndex = -1;

async function loadProjects() {
    try {
        const [projRes, statsRes] = await Promise.all([
            fetch('/api/projects'),
            fetch('/api/stats')
        ]);
        const projData = await projRes.json();
        const statsData = await statsRes.json();

        projects = projData.projects || [];
        updateStats(statsData);
        applyFilter();

        document.querySelector('.loading')?.remove();
    } catch (err) {
        document.getElementById('project-grid').innerHTML =
            `<div class="loading">Failed to load projects: ${err.message}</div>`;
    }
}

function updateStats(stats) {
    document.getElementById('project-count').textContent =
        `${stats.total} projects`;

    document.getElementById('stat-active').textContent =
        `● ${stats.active} active`;
    document.getElementById('stat-active').style.display = stats.active > 0 ? '' : 'none';

    document.getElementById('stat-dirty').textContent =
        `● ${stats.dirty} dirty`;
    document.getElementById('stat-dirty').style.display = stats.dirty > 0 ? '' : 'none';

    document.getElementById('stat-git').textContent =
        `● ${stats.git_repos} git`;

    document.getElementById('stat-context').textContent =
        `● ${stats.with_agent_context} contexts`;
    document.getElementById('stat-context').style.display = stats.with_agent_context > 0 ? '' : 'none';
}

function setFilter(filter) {
    currentFilter = filter;
    document.querySelectorAll('.filter-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.filter === filter);
    });
    applyFilter();
}

function onSearch() {
    applyFilter();
}

function applyFilter() {
    const query = document.getElementById('search').value.toLowerCase().trim();

    filtered = projects.filter(p => {
        // Type filter
        if (currentFilter === 'game' && p.type !== 'Godot') return false;
        if (currentFilter === 'tool' && !['Rust', 'Python', 'Chrome Ext', 'Nix', 'Docker'].includes(p.type)) return false;
        if (currentFilter === 'web' && !['TypeScript', 'JavaScript'].includes(p.type)) return false;
        if (currentFilter === 'active' && p.activity !== 'Active') return false;
        if (currentFilter === 'stable' && p.activity !== 'Stable') return false;
        if (currentFilter === 'stale' && p.activity !== 'Stale') return false;

        // Search
        if (query) {
            const name = p.name.toLowerCase();
            const type = p.type.toLowerCase();
            const phase = (p.agent?.current_phase || '').toLowerCase();
            if (!name.includes(query) && !type.includes(query) && !phase.includes(query)) {
                return false;
            }
        }

        return true;
    });

    document.getElementById('status-bar').textContent =
        `Showing ${filtered.length} of ${projects.length} projects`;

    renderGrid();
}

function renderGrid() {
    const grid = document.getElementById('project-grid');

    if (filtered.length === 0) {
        grid.innerHTML = `<div class="loading">No projects match your criteria</div>`;
        return;
    }

    grid.innerHTML = filtered.map((p, idx) => {
        const activityClass = p.activity.toLowerCase();
        const gitHealth = p.git ? p.git.health : 'clean';
        const gitBranch = p.git ? p.git.branch : '';
        const phase = p.agent?.current_phase || '';

        return `
            <div class="project-card" onclick="showDetail(${idx})">
                <div class="card-name">${escapeHtml(p.name)}</div>
                <div class="card-divider"></div>
                <div class="card-meta">
                    <span class="activity-badge ${activityClass}">${p.activity}</span>
                    <span class="type-badge">${p.type}</span>
                </div>
                <div class="card-size">
                    ${p.size_human} · ${p.file_count.toLocaleString()} files · ${p.days_since_modified}d ago
                </div>
                ${p.git ? `
                <div class="card-git">
                    <span class="git-status ${gitHealth}">${gitHealth === 'dirty' ? '⚡' : '✓'} ${gitHealth}</span>
                    <span class="git-branch">${escapeHtml(gitBranch)}</span>
                </div>
                ` : ''}
                ${p.agent?.description ? `<div class="card-desc">${escapeHtml(truncate(p.agent.description, 60))}</div>` : ''}
                ${phase ? `<div class="card-phase">${escapeHtml(phase)}</div>` : ''}
            </div>
        `;
    }).join('');
}

function showDetail(index) {
    const p = filtered[index];
    if (!p) return;

    currentDetailIndex = index;
    const modal = document.getElementById('detail-modal');
    const body = document.getElementById('detail-body');

    const git = p.git;
    const agent = p.agent;

    const taskHtml = agent?.current_task ? `
        <div class="detail-section">
            <h3>Current Task</h3>
            <div class="detail-row">
                <span class="value">${escapeHtml(agent.current_task)}</span>
            </div>
        </div>
    ` : '';

    const phaseHtml = agent?.current_phase ? `
        <div class="detail-section">
            <h3>Phase</h3>
            <div class="detail-row">
                <span class="value">${escapeHtml(agent.current_phase)}</span>
            </div>
        </div>
    ` : '';

    const nextStepsHtml = agent?.next_steps?.length ? `
        <div class="detail-section">
            <h3>Next Steps (${agent.next_steps.length})</h3>
            ${agent.next_steps.map(s => `<div class="detail-step">${escapeHtml(s)}</div>`).join('')}
        </div>
    ` : '';

    const blockersHtml = agent?.blockers?.length ? `
        <div class="detail-section">
            <h3>Blockers</h3>
            ${agent.blockers.map(s => `<div class="detail-blocker">${escapeHtml(s)}</div>`).join('')}
        </div>
    ` : '';

    const decisionsHtml = agent?.recent_decisions?.length ? `
        <div class="detail-section">
            <h3>Recent Decisions</h3>
            ${agent.recent_decisions.map(s => `<div class="detail-decision">${escapeHtml(s)}</div>`).join('')}
        </div>
    ` : '';

    const progressHtml = agent && agent.checklist_total > 0 ? `
        <div class="detail-section">
            <h3>Progress</h3>
            <div class="detail-row">
                <span class="value">${agent.checklist_done} / ${agent.checklist_total} tasks completed (${Math.round(agent.checklist_done / agent.checklist_total * 100)}%)</span>
            </div>
        </div>
    ` : '';

    body.innerHTML = `
        <div class="detail-header">
            <h2>${escapeHtml(p.name)}</h2>
            <div class="detail-path">${escapeHtml(p.path)}</div>
        </div>
        <div class="detail-section">
            <h3>Info</h3>
            <div class="detail-row">
                <span class="label">Type</span>
                <span class="value">${p.type}</span>
            </div>
            <div class="detail-row">
                <span class="label">Activity</span>
                <span class="value">${p.activity}</span>
            </div>
            <div class="detail-row">
                <span class="label">Size</span>
                <span class="value">${p.size_human} (${p.file_count.toLocaleString()} files)</span>
            </div>
            <div class="detail-row">
                <span class="label">Modified</span>
                <span class="value">${p.days_since_modified} days ago</span>
            </div>
            ${agent?.description ? `
            <div class="detail-row">
                <span class="label">Description</span>
                <span class="value" style="font-style: italic; color: var(--text-dim);">${escapeHtml(agent.description)}</span>
            </div>
            ` : ''}
        </div>
        ${git ? `
        <div class="detail-section">
            <h3>Git</h3>
            <div class="detail-row">
                <span class="label">Branch</span>
                <span class="value">${escapeHtml(git.branch)}</span>
            </div>
            <div class="detail-row">
                <span class="label">Status</span>
                <span class="value">${git.is_dirty ? '⚡ Dirty' : '✓ Clean'}
                    ${git.staged > 0 || git.unstaged > 0 || git.untracked > 0 ?
                        `(+${git.staged} staged, +${git.unstaged} unstaged, ${git.untracked} untracked)` : ''}
                </span>
            </div>
            ${git.ahead > 0 || git.behind > 0 ? `
            <div class="detail-row">
                <span class="label">Remote</span>
                <span class="value">${git.ahead} ahead · ${git.behind} behind</span>
            </div>
            ` : ''}
            ${git.last_commit_message ? `
            <div class="detail-row">
                <span class="label">Last commit</span>
                <span class="value">${escapeHtml(git.last_commit_message)}</span>
            </div>
            ` : ''}
            <div class="detail-row">
                <span class="label">Commits</span>
                <span class="value">${git.total_commits} total</span>
            </div>
        </div>
        ` : ''}
        ${phaseHtml}
        ${taskHtml}
        ${progressHtml}
        ${nextStepsHtml}
        ${blockersHtml}
        ${decisionsHtml}
        ${!agent ? `
        <div class="detail-section">
            <h3>Agent Context</h3>
            <div class="detail-row">
                <span class="value" style="color: var(--text-dim);">No agent context files found (CLAUDE.md, AGENTS.md, brief.md, etc.)</span>
            </div>
        </div>
        ` : ''}
    `;

    modal.classList.remove('hidden');
}

function closeDetail() {
    document.getElementById('detail-modal').classList.add('hidden');
}

async function refresh() {
    document.getElementById('status-bar').textContent = 'Rescanning...';
    try {
        const res = await fetch('/api/refresh', { method: 'POST' });
        const data = await res.json();
        document.getElementById('status-bar').textContent =
            `Rescanned — found ${data.projects_found} projects`;
        await loadProjects();
    } catch (err) {
        document.getElementById('status-bar').textContent =
            `Refresh failed: ${err.message}`;
    }
}

function truncate(str, max) {
    if (!str || str.length <= max) return str || '';
    return str.slice(0, max - 1) + '…';
}

function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;').replace(/'/g, '&#039;');
}

// ─── Scan Roots Management ────────────────────────────────

async function loadRoots() {
    try {
        const res = await fetch('/api/roots');
        const data = await res.json();
        renderRoots(data.roots || []);
    } catch (err) {
        console.error('Failed to load roots:', err);
    }
}

function renderRoots(roots) {
    // Roots bar chips
    const list = document.getElementById('roots-list');
    list.innerHTML = roots.map(r =>
        `<span class="root-chip">${escapeHtml(r.label)}</span>`
    ).join('');

    // Roots panel list
    const body = document.getElementById('roots-panel-body');
    if (roots.length === 0) {
        body.innerHTML = '<div style="color: var(--text-dim); font-size: 13px; padding: 8px;">No scan roots configured. Add one below.</div>';
        return;
    }

    body.innerHTML = roots.map((r, i) => `
        <div class="root-item">
            <span class="root-label">${escapeHtml(r.label)}</span>
            <span class="root-path">${escapeHtml(r.path)}</span>
            <button class="root-enabled ${r.enabled ? 'active' : ''}" onclick="toggleRoot(${i})">
                ${r.enabled ? '● active' : '○ disabled'}
            </button>
            <button class="root-remove" onclick="removeRoot(${i})">✕</button>
        </div>
    `).join('');
}

function toggleRootsPanel() {
    const panel = document.getElementById('roots-panel');
    panel.classList.toggle('hidden');
    if (!panel.classList.contains('hidden')) {
        loadRoots();
    }
}

async function addRoot() {
    const input = document.getElementById('new-root-path');
    const path = input.value.trim();
    if (!path) return;

    try {
        const res = await fetch('/api/roots', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path })
        });
        const data = await res.json();
        if (data.status === 'ok') {
            input.value = '';
            await loadRoots();
            await loadProjects();
        } else {
            alert('Error: ' + (data.message || 'unknown'));
        }
    } catch (err) {
        alert('Failed to add root: ' + err.message);
    }
}

async function removeRoot(index) {
    try {
        const res = await fetch(`/api/roots/${index}`, { method: 'DELETE' });
        const data = await res.json();
        if (data.status === 'ok') {
            await loadRoots();
            await loadProjects();
        }
    } catch (err) {
        alert('Failed to remove root: ' + err.message);
    }
}

async function toggleRoot(index) {
    // Fetch current state to toggle
    try {
        const rootsRes = await fetch('/api/roots');
        const rootsData = await rootsRes.json();
        const root = rootsData.roots[index];
        if (!root) return;

        const res = await fetch(`/api/roots/${index}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ enabled: !root.enabled })
        });
        const data = await res.json();
        if (data.status === 'ok') {
            await loadRoots();
            await loadProjects();
        }
    } catch (err) {
        alert('Failed to toggle root: ' + err.message);
    }
}

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        closeDetail();
        const panel = document.getElementById('roots-panel');
        if (!panel.classList.contains('hidden')) {
            panel.classList.add('hidden');
        }
    }
    if (e.key === '/' && !e.ctrlKey && !e.metaKey) {
        const search = document.getElementById('search');
        if (document.activeElement !== search) {
            e.preventDefault();
            search.focus();
        }
    }
});

// Load on startup
loadProjects();
loadRoots();

// Auto-refresh every 60 seconds
setInterval(loadProjects, 60000);
