// ── Data & State ────────────────────────────────────────────────
let DATA = null;
let filteredPatterns = [];
let selectedPattern = null;
let activeProofStep = null;

// ── Sudoku graph adjacency (precomputed) ────────────────────────
const NEIGHBORS = new Array(81);
for (let v = 0; v < 81; v++) {
  NEIGHBORS[v] = new Set();
  const r = Math.floor(v / 9), c = v % 9;
  const br = 3 * Math.floor(r / 3), bc = 3 * Math.floor(c / 3);
  for (let u = 0; u < 81; u++) {
    if (u === v) continue;
    const r2 = Math.floor(u / 9), c2 = u % 9;
    if (r2 === r || c2 === c || (3 * Math.floor(r2 / 3) === br && 3 * Math.floor(c2 / 3) === bc)) {
      NEIGHBORS[v].add(u);
    }
  }
}

// ── Init ────────────────────────────────────────────────────────
async function init() {
  const resp = await fetch('data/patterns.json');
  DATA = await resp.json();
  populateCounts();
  applyFilters();
  setupEventListeners();
}

function populateCounts() {
  for (const btn of document.querySelectorAll('#size-buttons .btn[data-size]')) {
    const s = btn.dataset.size;
    if (s !== 'all') {
      btn.querySelector('.count').textContent = `(${DATA.sizes[s] || 0})`;
    }
  }
}

// ── Filters ─────────────────────────────────────────────────────
function applyFilters() {
  const sizeBtn = document.querySelector('#size-buttons .btn.active');
  const branchBtn = document.querySelector('#branch-buttons .btn.active');
  const teBtn = document.querySelector('#te-buttons .btn.active');
  const bandBtn = document.querySelector('#band-buttons .btn.active');
  const degFilter = document.getElementById('degree-filter').value;
  const searchId = document.getElementById('search-id').value.trim().toUpperCase();

  const sizeFilter = sizeBtn?.dataset.size || 'all';
  const branchFilter = branchBtn?.dataset.branches || 'all';
  const teFilter = teBtn?.dataset.te || 'all';
  const bandFilter = bandBtn?.dataset.bands || 'all';

  filteredPatterns = DATA.patterns.filter(p => {
    if (sizeFilter !== 'all' && p.size !== parseInt(sizeFilter)) return false;
    if (branchFilter !== 'all' && (p.proof?.branches ?? 0) !== parseInt(branchFilter)) return false;
    if (teFilter !== 'all' && (p.te_depth ?? 0) !== parseInt(teFilter)) return false;
    if (bandFilter !== 'all' && p.num_bands !== parseInt(bandFilter)) return false;
    if (degFilter !== 'all' && p.min_degree < parseInt(degFilter)) return false;
    if (searchId && !p.id.toUpperCase().includes(searchId)) return false;
    return true;
  });

  document.getElementById('filter-stats').textContent =
    `Showing ${filteredPatterns.length} of ${DATA.total_patterns} patterns`;

  renderPatternList();
}

// ── Pattern List ────────────────────────────────────────────────
function renderPatternList() {
  const body = document.getElementById('pattern-list-body');
  // Use DocumentFragment for performance with 1500+ patterns
  const fragment = document.createDocumentFragment();

  for (const p of filteredPatterns) {
    const row = document.createElement('div');
    row.className = 'pattern-row' + (selectedPattern?.id === p.id ? ' selected' : '');
    row.dataset.id = p.id;

    const branches = p.proof?.branches ?? 0;
    const badgeClass = branches > 0 ? 'branching' : 'simple';
    const badgeText = branches > 0 ? `${branches}br` : 'simple';

    row.innerHTML =
      `<span class="col-id">${p.id}</span>` +
      `<span class="col-edges">${p.num_edges}</span>` +
      `<span class="col-deg">[${p.degree_sequence.join(',')}]</span>` +
      `<span class="col-proof"><span class="proof-badge ${badgeClass}">${badgeText}</span></span>`;

    row.addEventListener('click', () => selectPattern(p));
    fragment.appendChild(row);
  }

  body.innerHTML = '';
  body.appendChild(fragment);
}

function selectPattern(p) {
  selectedPattern = p;
  activeProofStep = null;

  // Update list selection
  for (const row of document.querySelectorAll('.pattern-row')) {
    row.classList.toggle('selected', row.dataset.id === p.id);
  }

  document.getElementById('detail-placeholder').style.display = 'none';
  document.getElementById('detail-view').style.display = 'block';

  renderDetail(p);
}

// ── Detail View ─────────────────────────────────────────────────
function renderDetail(p) {
  document.getElementById('detail-title').textContent = `Pattern ${p.id}`;

  const meta = document.getElementById('detail-meta');
  meta.innerHTML = [
    metaItem('Size', p.size),
    metaItem('Edges', p.num_edges),
    metaItem('Degrees', `[${p.degree_sequence.join(', ')}]`),
    metaItem('Bands', p.num_bands),
    metaItem('Rows', p.rows_used.map(r => r + 1).join(', ')),
    metaItem('T&amp;E depth', p.te_depth ?? '?'),
    metaItem('Proof depth', p.proof?.depth ?? '?'),
    metaItem('Diamonds', p.proof?.diamonds ?? '?'),
    metaItem('Branches', p.proof?.branches ?? 0),
  ].join('');

  document.getElementById('bitstring-text').textContent = p.bitstring;

  renderGrid(p);
  renderProof(p);
}

function metaItem(label, value) {
  return `<div class="meta-item"><span class="meta-label">${label}:</span><span class="meta-value">${value}</span></div>`;
}



// ── SVG Grid ────────────────────────────────────────────────────
const CELL_SIZE = 38;
const GRID_PAD = 9;

function renderGrid(p, highlights) {
  const svg = document.getElementById('sudoku-grid');
  const showEdges = document.getElementById('show-edges').checked;
  const showLabels = document.getElementById('show-labels').checked;

  const cellIndices = p.cell_indices;
  const edges = p.edges;

  const cellSet = new Set(cellIndices);

  let html = '';

  // Background cells
  for (let r = 0; r < 9; r++) {
    for (let c = 0; c < 9; c++) {
      const v = r * 9 + c;
      const x = GRID_PAD + c * CELL_SIZE;
      const y = GRID_PAD + r * CELL_SIZE;
      const isActive = cellSet.has(v);

      let cls = isActive ? 'cell-active' : 'cell-inactive';
      if (highlights && isActive) {
        const h = highlights[v];
        if (h) cls += ' ' + h;
      }

      html += `<rect x="${x}" y="${y}" width="${CELL_SIZE}" height="${CELL_SIZE}" class="${cls}" data-cell="${v}"/>`;
    }
  }

  // Edges between pattern cells
  if (showEdges) {
    for (const [i, j] of edges) {
      const u = cellIndices[i];
      const v = cellIndices[j];
      const ur = Math.floor(u / 9), uc = u % 9;
      const vr = Math.floor(v / 9), vc = v % 9;
      const x1 = GRID_PAD + uc * CELL_SIZE + CELL_SIZE / 2;
      const y1 = GRID_PAD + ur * CELL_SIZE + CELL_SIZE / 2;
      const x2 = GRID_PAD + vc * CELL_SIZE + CELL_SIZE / 2;
      const y2 = GRID_PAD + vr * CELL_SIZE + CELL_SIZE / 2;

      let cls = 'edge-line';
      if (highlights) {
        const hu = highlights[u], hv = highlights[v];
        if (hu && hv) cls += ' highlighted';
      }

      html += `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" class="${cls}"/>`;
    }
  }

  // Grid lines
  for (let i = 0; i <= 9; i++) {
    const pos = GRID_PAD + i * CELL_SIZE;
    const cls = (i % 3 === 0) ? 'grid-line-thick' : 'grid-line-thin';
    html += `<line x1="${GRID_PAD}" y1="${pos}" x2="${GRID_PAD + 9 * CELL_SIZE}" y2="${pos}" class="${cls}"/>`;
    html += `<line x1="${pos}" y1="${GRID_PAD}" x2="${pos}" y2="${GRID_PAD + 9 * CELL_SIZE}" class="${cls}"/>`;
  }

  // Border
  html += `<rect x="${GRID_PAD}" y="${GRID_PAD}" width="${9 * CELL_SIZE}" height="${9 * CELL_SIZE}" class="grid-border"/>`;

  // Cell labels
  if (showLabels) {
    for (const v of cellIndices) {
      const r = Math.floor(v / 9), c = v % 9;
      const x = GRID_PAD + c * CELL_SIZE + CELL_SIZE / 2;
      const y = GRID_PAD + r * CELL_SIZE + CELL_SIZE / 2;
      html += `<text x="${x}" y="${y}" class="cell-label">r${r + 1}c${c + 1}</text>`;
    }
  }

  svg.innerHTML = html;
}

// ── Proof Renderer ──────────────────────────────────────────────
function renderProof(p) {
  const container = document.getElementById('proof-view');
  if (!p.proof || !p.proof.tree || p.proof.tree.length === 0) {
    container.innerHTML = '<div class="proof-preamble">No proof data available.</div>';
    return;
  }

  let html = '<div class="proof-preamble">Assume for contradiction it is 3-colorable.</div>';
  html += renderProofSteps(p.proof.tree, 0);
  if (p.proof.complete) {
    html += '<div class="proof-conclusion">Therefore the pattern is not 3-colorable. □</div>';
  }
  container.innerHTML = html;

  // Add click handlers to proof steps
  for (const el of container.querySelectorAll('.proof-step')) {
    el.addEventListener('click', () => highlightProofStep(p, el));
  }
}

function renderProofSteps(steps, depth) {
  let html = '';
  for (const step of steps) {
    if (step.type === 'diamond') {
      html += renderDiamondStep(step, depth);
    } else if (step.type === 'k4') {
      html += renderK4Step(step, depth);
    } else if (step.type === 'branch') {
      html += renderBranchStep(step, depth);
    }
  }
  return html;
}

function renderDiamondStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'diamond',
    tip_a: step.tip_a,
    tip_b: step.tip_b,
    spine_u: step.spine_u,
    spine_v: step.spine_v,
    vertices: step.vertices,
  }));

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-diamond">Diamond</span> ` +
    `{${step.vertices.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ')}} ` +
    `(spine <span class="step-vertex">${esc(step.spine_u)}</span>—<span class="step-vertex">${esc(step.spine_v)}</span>)` +
    `<br><span class="step-identify">→ color(${esc(step.tip_a)}) = color(${esc(step.tip_b)}). Identify.</span>` +
    `</div>`;
}

function renderK4Step(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'k4',
    vertices: step.vertices,
  }));

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">K₄</span> on ` +
    `{${step.vertices.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ')}}. ` +
    `<span class="step-k4">Contradiction.</span>` +
    `</div>`;
}

function renderBranchStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'branch',
    vertex_a: step.vertex_a,
    vertex_b: step.vertex_b,
  }));

  let html = `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-branch">Branch</span> on ` +
    `<span class="step-vertex">${esc(step.vertex_a)}</span>, ` +
    `<span class="step-vertex">${esc(step.vertex_b)}</span>:` +
    `</div>`;

  html += `<div class="branch-block case-a">`;
  html += `<div class="step-case-label">Case A: color(${esc(step.vertex_a)}) = color(${esc(step.vertex_b)}). Identify.</div>`;
  html += renderProofSteps(step.case_a, depth + 1);
  html += `</div>`;

  html += `<div class="branch-block case-b">`;
  html += `<div class="step-case-label">Case B: color(${esc(step.vertex_a)}) ≠ color(${esc(step.vertex_b)}). Add edge.</div>`;
  html += renderProofSteps(step.case_b, depth + 1);
  html += `</div>`;

  html += `<div class="step-both-cases">Both cases contradict 3-colorability.</div>`;

  return html;
}

// ── Grid highlighting from proof steps ──────────────────────────
function highlightProofStep(pattern, el) {
  // Toggle active
  const wasActive = el.classList.contains('active');
  for (const s of document.querySelectorAll('.proof-step.active')) {
    s.classList.remove('active');
  }

  if (wasActive) {
    activeProofStep = null;
    renderGrid(pattern);
    return;
  }

  el.classList.add('active');
  const stepData = JSON.parse(el.dataset.step);
  activeProofStep = stepData;

  const highlights = {};

  if (stepData.type === 'diamond') {
    // Highlight the 4 vertices of the diamond
    for (const vName of stepData.vertices) {
      for (const cell of parseCellsFromLabel(vName)) {
        highlights[cell] = 'highlighted';
      }
    }
    // Mark tips specially
    for (const cell of parseCellsFromLabel(stepData.tip_a)) {
      highlights[cell] = 'merged-a';
    }
    for (const cell of parseCellsFromLabel(stepData.tip_b)) {
      highlights[cell] = 'merged-b';
    }
    // Mark spine
    for (const cell of parseCellsFromLabel(stepData.spine_u)) {
      highlights[cell] = 'spine';
    }
    for (const cell of parseCellsFromLabel(stepData.spine_v)) {
      highlights[cell] = 'spine';
    }
  } else if (stepData.type === 'k4') {
    for (const vName of stepData.vertices) {
      for (const cell of parseCellsFromLabel(vName)) {
        highlights[cell] = 'k4';
      }
    }
  } else if (stepData.type === 'branch') {
    for (const cell of parseCellsFromLabel(stepData.vertex_a)) {
      highlights[cell] = 'merged-a';
    }
    for (const cell of parseCellsFromLabel(stepData.vertex_b)) {
      highlights[cell] = 'merged-b';
    }
  }

  renderGrid(pattern, highlights);
}

// Parse "r3c5" or "[r1c2=r3c5=r4c1]" into cell indices
function parseCellsFromLabel(label) {
  const cells = [];
  const regex = /r(\d+)c(\d+)/g;
  let m;
  while ((m = regex.exec(label)) !== null) {
    const r = parseInt(m[1]) - 1; // 1-based to 0-based
    const c = parseInt(m[2]) - 1;
    if (r >= 0 && r < 9 && c >= 0 && c < 9) {
      cells.push(r * 9 + c);
    }
  }
  return cells;
}

// ── Event Listeners ─────────────────────────────────────────────
function setupEventListeners() {
  // Size filter buttons
  for (const btn of document.querySelectorAll('#size-buttons .btn')) {
    btn.addEventListener('click', () => {
      document.querySelector('#size-buttons .btn.active')?.classList.remove('active');
      btn.classList.add('active');
      applyFilters();
    });
  }

  // Branch filter buttons
  for (const btn of document.querySelectorAll('#branch-buttons .btn')) {
    btn.addEventListener('click', () => {
      document.querySelector('#branch-buttons .btn.active')?.classList.remove('active');
      btn.classList.add('active');
      applyFilters();
    });
  }

  // T&E depth filter buttons
  for (const btn of document.querySelectorAll('#te-buttons .btn')) {
    btn.addEventListener('click', () => {
      document.querySelector('#te-buttons .btn.active')?.classList.remove('active');
      btn.classList.add('active');
      applyFilters();
    });
  }

  // Band filter buttons
  for (const btn of document.querySelectorAll('#band-buttons .btn')) {
    btn.addEventListener('click', () => {
      document.querySelector('#band-buttons .btn.active')?.classList.remove('active');
      btn.classList.add('active');
      applyFilters();
    });
  }

  // Degree filter
  document.getElementById('degree-filter').addEventListener('change', applyFilters);

  // Search
  document.getElementById('search-id').addEventListener('input', applyFilters);

  // Grid controls
  document.getElementById('show-edges').addEventListener('change', () => {
    if (selectedPattern) renderGrid(selectedPattern);
  });
  document.getElementById('show-labels').addEventListener('change', () => {
    if (selectedPattern) renderGrid(selectedPattern);
  });

  // Copy bitstring
  document.getElementById('copy-bitstring').addEventListener('click', () => {
    const bs = document.getElementById('bitstring-text').textContent;
    navigator.clipboard.writeText(bs).then(() => showCopyToast('Bitstring copied'));
  });

  // Copy grid screenshot
  document.getElementById('copy-grid').addEventListener('click', () => {
    copyGridToClipboard();
  });

  document.getElementById('copy-proof').addEventListener('click', () => {
    if (!selectedPattern) return;
    const text = proofToText(selectedPattern);
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text)
        .then(() => showCopyToast('Proof copied'))
        .catch(() => fallbackCopy(text));
    } else {
      fallbackCopy(text);
    }
  });

  // Keyboard navigation
  document.addEventListener('keydown', handleKeyboard);
}

function handleKeyboard(e) {
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;

  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    if (!filteredPatterns.length) return;
    const currentIdx = selectedPattern
      ? filteredPatterns.findIndex(p => p.id === selectedPattern.id) : -1;
    let newIdx;
    if (e.key === 'ArrowDown') {
      newIdx = currentIdx < filteredPatterns.length - 1 ? currentIdx + 1 : 0;
    } else {
      newIdx = currentIdx > 0 ? currentIdx - 1 : filteredPatterns.length - 1;
    }
    selectPattern(filteredPatterns[newIdx]);
    // Scroll into view
    const row = document.querySelector(`.pattern-row[data-id="${filteredPatterns[newIdx].id}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }
}

// ── Utilities ───────────────────────────────────────────────────
function esc(str) {
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeAttr(str) {
  return str.replace(/&/g, '&amp;').replace(/'/g, '&#39;').replace(/"/g, '&quot;');
}

// ── Copy grid to clipboard ──────────────────────────────────────
function proofToText(p) {
  if (!p.proof || !p.proof.tree || p.proof.tree.length === 0) return 'No proof data.';
  const lines = ['Assume for contradiction it is 3-colorable.'];
  function walk(steps, depth) {
    const indent = '\t'.repeat(depth);
    for (const s of steps) {
      if (s.type === 'diamond') {
        lines.push(`${indent}${s.step}. Diamond {${s.vertices.join(', ')}} (spine ${s.spine_u}\u2014${s.spine_v})`);
        lines.push(`${indent}   \u2192 color(${s.tip_a}) = color(${s.tip_b}). Identify.`);
      } else if (s.type === 'k4') {
        lines.push(`${indent}${s.step}. K4 on {${s.vertices.join(', ')}}. Contradiction.`);
      } else if (s.type === 'branch') {
        lines.push(`${indent}${s.step}. Branch on ${s.vertex_a}, ${s.vertex_b}:`);
        lines.push(`${indent}\tCase A: color(${s.vertex_a}) = color(${s.vertex_b}). Identify.`);
        walk(s.case_a, depth + 1);
        lines.push(`${indent}\tCase B: color(${s.vertex_a}) \u2260 color(${s.vertex_b}). Add edge.`);
        walk(s.case_b, depth + 1);
        lines.push(`${indent}Both cases contradict 3-colorability.`);
      }
    }
  }
  walk(p.proof.tree, 0);
  if (p.proof.complete) lines.push('Therefore the pattern is not 3-colorable. \u25a1');
  return lines.join('\n');
}

function copyGridToClipboard() {
  const svg = document.getElementById('sudoku-grid');
  // Clone SVG and inline computed styles for standalone rendering
  const clone = svg.cloneNode(true);
  // Add a style block with all the necessary rules
  const styleEl = document.createElementNS('http://www.w3.org/2000/svg', 'style');
  styleEl.textContent = `
    .cell-inactive { fill: #0d1117; }
    .cell-active { fill: #1f6feb; opacity: 0.7; }
    .cell-active.highlighted { fill: #f0883e; opacity: 0.9; }
    .cell-active.merged-a { fill: #da3633; opacity: 0.85; }
    .cell-active.merged-b { fill: #da3633; opacity: 0.85; }
    .cell-active.spine { fill: #a371f7; opacity: 0.85; }
    .cell-active.k4 { fill: #f85149; opacity: 0.95; }
    .grid-line-thin { stroke: #30363d; stroke-width: 0.5; }
    .grid-line-thick { stroke: #484f58; stroke-width: 2; }
    .grid-border { stroke: #6e7681; stroke-width: 2.5; fill: none; }
    .edge-line { stroke: #484f58; stroke-width: 0.6; opacity: 0.4; }
    .edge-line.highlighted { stroke: #f0883e; stroke-width: 1.5; opacity: 0.8; }
    .cell-label { font-size: 8px; fill: #e6edf3; text-anchor: middle; dominant-baseline: central; font-family: monospace; }
  `;
  clone.insertBefore(styleEl, clone.firstChild);
  // Add background rect
  const bg = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
  bg.setAttribute('width', '360');
  bg.setAttribute('height', '360');
  bg.setAttribute('fill', '#161b22');
  clone.insertBefore(bg, styleEl.nextSibling);

  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  const svgData = new XMLSerializer().serializeToString(clone);

  const canvas = document.createElement('canvas');
  const scale = 2;
  canvas.width = 360 * scale;
  canvas.height = 360 * scale;
  const ctx = canvas.getContext('2d');
  ctx.scale(scale, scale);

  const img = new Image();
  const blob = new Blob([svgData], { type: 'image/svg+xml;charset=utf-8' });
  const url = URL.createObjectURL(blob);

  img.onload = function () {
    ctx.drawImage(img, 0, 0, 360, 360);
    URL.revokeObjectURL(url);

    canvas.toBlob(function (pngBlob) {
      if (!pngBlob) {
        showCopyToast('Failed to create image');
        return;
      }
      navigator.clipboard.write([
        new ClipboardItem({ 'image/png': pngBlob })
      ]).then(() => {
        showCopyToast('Grid image copied');
      }).catch(() => {
        showCopyToast('Copy failed — try right-click → save');
      });
    }, 'image/png');
  };

  img.src = url;
}

function fallbackCopy(text) {
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.style.position = 'fixed';
  ta.style.opacity = '0';
  document.body.appendChild(ta);
  ta.select();
  document.execCommand('copy');
  document.body.removeChild(ta);
  showCopyToast('Proof copied');
}

function showCopyToast(msg) {
  let toast = document.querySelector('.copy-toast');
  if (!toast) {
    toast = document.createElement('div');
    toast.className = 'copy-toast';
    document.body.appendChild(toast);
  }
  toast.textContent = msg;
  toast.classList.add('visible');
  clearTimeout(toast._timer);
  toast._timer = setTimeout(() => toast.classList.remove('visible'), 1500);
}

// ── Start ───────────────────────────────────────────────────────
init();
