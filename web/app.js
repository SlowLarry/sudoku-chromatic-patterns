// ── Data & State ────────────────────────────────────────────────
let DATA = null;
let filteredPatterns = [];
let selectedPattern = null;
let activeProofStep = null;

// ── Auxiliary color palette for accumulated diamond merges ──────
const AUX_COLORS = ['color-a', 'color-b', 'color-c', 'color-d'];

// ── Union-Find for tracking diamond merge groups ────────────────
class UnionFind {
  constructor() { this.parent = new Map(); }
  find(x) {
    if (!this.parent.has(x)) { this.parent.set(x, x); return x; }
    let r = x;
    while (this.parent.get(r) !== r) r = this.parent.get(r);
    let c = x;
    while (c !== r) { const n = this.parent.get(c); this.parent.set(c, r); c = n; }
    return r;
  }
  union(a, b) { this.parent.set(this.find(a), this.find(b)); }
  clone() {
    const c = new UnionFind();
    for (const [k, v] of this.parent) c.parent.set(k, v);
    return c;
  }
}

/**
 * Walk the proof tree, accumulating diamond merges and virtual edges
 * up to and including the step whose DOM element === targetEl.
 * Returns { highlights, virtualEdges } where highlights is a map
 * (cell index → CSS class) for accumulated merge colors, and
 * virtualEdges is an array of [u, v, cssClass] for accumulated
 * virtual edges from prior steps (not including the target step).
 *
 * Branch scoping: merges/edges from before a branch carry into both
 * cases, but those inside case A don't leak into case B.
 */
function computeAccumulatedColors(proofContainer, targetEl) {
  // Collect all proof-step elements in DOM order (depth-first)
  const allSteps = Array.from(proofContainer.querySelectorAll('.proof-step'));
  const targetIdx = allSteps.indexOf(targetEl);
  if (targetIdx < 0) return { highlights: {}, virtualEdges: [] };

  const uf = new UnionFind();
  const highlights = {};
  let colorCounter = 0;

  // Accumulated virtual edges from prior steps (keyed for dedup)
  const virtSet = new Set();    // 'min,max' keys
  const virtList = [];          // [[u, v, ''], ...]

  function addVirtEdge(u, v) {
    const key = Math.min(u, v) + ',' + Math.max(u, v);
    if (!virtSet.has(key) && !NEIGHBORS[u].has(v)) {
      virtSet.add(key);
      virtList.push([u, v, '']);
    }
  }

  // Build the branch ancestry path to the target
  function getBranchPath(el) {
    const path = [];
    let cur = el.parentElement;
    while (cur && cur !== proofContainer) {
      if (cur.classList.contains('branch-block')) {
        path.unshift(cur);
      }
      cur = cur.parentElement;
    }
    return path;
  }

  const targetPath = getBranchPath(targetEl);

  // For each step, check if it's on the path to the target
  function isOnTargetPath(el) {
    let cur = el.parentElement;
    while (cur && cur !== proofContainer) {
      if (cur.classList.contains('branch-block')) {
        if (!targetPath.includes(cur)) return false;
      }
      cur = cur.parentElement;
    }
    return true;
  }

  for (let i = 0; i <= targetIdx; i++) {
    const el = allSteps[i];
    if (!isOnTargetPath(el)) continue;

    let stepData;
    try { stepData = JSON.parse(el.dataset.step); } catch { continue; }

    if (stepData.type === 'diamond') {
      const cellsA = parseCellsFromLabel(stepData.tip_a);
      const cellsB = parseCellsFromLabel(stepData.tip_b);
      if (cellsA.length > 0 && cellsB.length > 0) {
        const allCells = [...cellsA, ...cellsB];
        for (let j = 1; j < allCells.length; j++) {
          uf.union(allCells[0], allCells[j]);
        }
      }
    }

    // Accumulate virtual edges from prior steps (not the target itself)
    if (i < targetIdx) {
      if (stepData.type === 'circular_ladder' && stepData.satellites) {
        const satCells = [];
        for (const [, name] of stepData.satellites) {
          satCells.push(...parseCellsFromLabel(name));
        }
        for (let si = 0; si < satCells.length; si++) {
          for (let sj = si + 1; sj < satCells.length; sj++) {
            addVirtEdge(satCells[si], satCells[sj]);
          }
        }
      } else if (stepData.type === 'set_equivalence' && !stepData.is_contradiction) {
        const lhsCells = stepData.lhs ? stepData.lhs.flatMap(v => parseCellsFromLabel(v)) : [];
        const rhsCells = stepData.rhs ? stepData.rhs.flatMap(v => parseCellsFromLabel(v)) : [];
        if (lhsCells.length >= 2 || rhsCells.length >= 2) {
          for (const group of [lhsCells, rhsCells]) {
            for (let si = 0; si < group.length; si++) {
              for (let sj = si + 1; sj < group.length; sj++) {
                addVirtEdge(group[si], group[sj]);
              }
            }
          }
        }
      } else if (stepData.type === 'parity_transport_deduction' && !stepData.forced_same) {
        const cellsA = parseCellsFromLabel(stepData.cell_a);
        const cellsB = parseCellsFromLabel(stepData.cell_b);
        for (const a of cellsA) {
          for (const b of cellsB) {
            addVirtEdge(a, b);
          }
        }
      } else if (stepData.type === 'branch') {
        const brA = parseCellsFromLabel(stepData.vertex_a);
        const brB = parseCellsFromLabel(stepData.vertex_b);
        // Find the Case A block that follows this branch step
        let caseABlock = el.nextElementSibling;
        while (caseABlock && !caseABlock.classList.contains('case-a')) {
          caseABlock = caseABlock.nextElementSibling;
        }
        const inCaseA = caseABlock && caseABlock.contains(targetEl);
        if (inCaseA) {
          // Case A: cells identified (same color) → merge
          for (const a of brA) {
            for (const b of brB) {
              uf.union(a, b);
            }
          }
        } else {
          // Case B: cells different colors → virtual edge
          for (const a of brA) {
            for (const b of brB) {
              addVirtEdge(a, b);
            }
          }
        }
      }
    }
  }

  // Assign colors to each merge group
  const groupMap = new Map();
  for (const key of uf.parent.keys()) {
    const root = uf.find(key);
    if (!groupMap.has(root)) groupMap.set(root, []);
    groupMap.get(root).push(key);
  }

  for (const [root, cells] of groupMap) {
    if (cells.length < 2) continue;
    const colorClass = AUX_COLORS[colorCounter % AUX_COLORS.length];
    colorCounter++;
    for (const cell of cells) {
      highlights[cell] = colorClass;
    }
  }

  return { highlights, virtualEdges: virtList };
}

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
  const diffBtn = document.querySelector('#diff-buttons .btn.active');

  const degFilter = document.getElementById('degree-filter').value;
  const searchId = document.getElementById('search-id').value.trim().toUpperCase();

  const sizeFilter = sizeBtn?.dataset.size || 'all';
  const branchFilter = branchBtn?.dataset.branches || 'all';
  const diffFilter = diffBtn?.dataset.diff || 'all';


  filteredPatterns = DATA.patterns.filter(p => {
    if (sizeFilter !== 'all' && p.size !== parseInt(sizeFilter)) return false;
    if (branchFilter !== 'all' && (p.proof?.branches ?? 0) !== parseInt(branchFilter)) return false;
    if (diffFilter !== 'all' && getDifficulty(p) !== diffFilter) return false;

    if (degFilter !== 'all' && p.min_degree < parseInt(degFilter)) return false;
    if (searchId && !p.id.toUpperCase().includes(searchId)) return false;
    return true;
  });

  document.getElementById('filter-stats').textContent =
    `Showing ${filteredPatterns.length} of ${DATA.total_patterns} patterns`;

  renderPatternList();
}

// ── Pattern List ────────────────────────────────────────────────
function getDifficulty(p) {
  const gb = p.proof?.greedy_branches ?? 0;
  const gow = p.proof?.greedy_odd_wheels ?? 0;
  const gbh = p.proof?.greedy_bridged_hexagons ?? 0;
  const gcl = p.proof?.greedy_circular_ladders ?? 0;
  const gse = p.proof?.greedy_set_equivalences ?? 0;
  const gpt = p.proof?.greedy_parity_transports ?? 0;
  const gxw = p.proof?.greedy_pigeonhole_xwings ?? 0;
  if (gb > 0) return 'branch';
  if (gow > 0) return 'oddagon';
  if (gbh > 0) return 'hexagon';
  if (gcl > 0) return 'ladder';
  if (gxw > 0) return 'xwing';
  if (gse > 0) return 'set';
  if (gpt > 0) return 'parity';
  return 'diamond';
}

function renderPatternList() {
  const body = document.getElementById('pattern-list-body');
  // Use DocumentFragment for performance with 1500+ patterns
  const fragment = document.createDocumentFragment();

  for (const p of filteredPatterns) {
    const row = document.createElement('div');
    row.className = 'pattern-row' + (selectedPattern?.id === p.id ? ' selected' : '');
    row.dataset.id = p.id;

    const diff = getDifficulty(p);
    const badgeClass = diff;
    const badgeText = diff === 'branch' ? `${p.proof?.greedy_branches}br` : diff;

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
    metaItem('Rows', p.rows_used.map(r => r + 1).join(', ')),
    metaItem('Difficulty', getDifficulty(p)),
    metaItem('Proof depth', p.proof?.depth ?? '?'),
    metaItem('Diamonds', p.proof?.diamonds ?? '?'),
    metaItem('Odd wheels', p.proof?.odd_wheels ?? 0),
    metaItem('Circ. ladders', p.proof?.circular_ladders ?? 0),
    metaItem('Bridged hex.', p.proof?.bridged_hexagons ?? 0),
    metaItem('SET equiv.', p.proof?.set_equivalences ?? 0),
    metaItem('Parity', p.proof?.parity_transports ?? 0),
    metaItem('X-wings', p.proof?.pigeonhole_xwings ?? 0),
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

function renderGrid(p, highlights, edgeHighlights, virtualEdges) {
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

  // Edges between pattern cells (drawn as arcs)
  if (showEdges) {
    // Track how many edges share the same cell pair for arc curvature
    const edgeCount = new Map();
    for (const [i, j] of edges) {
      const key = Math.min(cellIndices[i], cellIndices[j]) + ',' + Math.max(cellIndices[i], cellIndices[j]);
      edgeCount.set(key, (edgeCount.get(key) || 0) + 1);
    }
    const edgeSeen = new Map();

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
      const edgeKey = Math.min(u, v) + ',' + Math.max(u, v);
      if (edgeHighlights && edgeHighlights.has(edgeKey)) {
        cls += ' ' + edgeHighlights.get(edgeKey);
      } else if (highlights) {
        const hu = highlights[u], hv = highlights[v];
        if (hu && hv) cls += ' highlighted';
      }

      // Compute arc curvature: perpendicular offset proportional to distance
      const dx = x2 - x1, dy = y2 - y1;
      const dist = Math.sqrt(dx * dx + dy * dy);
      // Perpendicular unit vector
      const px = -dy / dist, py = dx / dist;
      // Curvature: gentle bow, scaled to distance
      const bow = dist * 0.18;
      const mx = (x1 + x2) / 2 + px * bow;
      const my = (y1 + y2) / 2 + py * bow;

      html += `<path d="M${x1},${y1} Q${mx},${my} ${x2},${y2}" class="${cls}"/>`;
    }

    // Virtual edges (deduced, not in the original pattern)
    if (virtualEdges) {
      for (const [vu, vv, vcls] of virtualEdges) {
        const vur = Math.floor(vu / 9), vuc = vu % 9;
        const vvr = Math.floor(vv / 9), vvc = vv % 9;
        const vx1 = GRID_PAD + vuc * CELL_SIZE + CELL_SIZE / 2;
        const vy1 = GRID_PAD + vur * CELL_SIZE + CELL_SIZE / 2;
        const vx2 = GRID_PAD + vvc * CELL_SIZE + CELL_SIZE / 2;
        const vy2 = GRID_PAD + vvr * CELL_SIZE + CELL_SIZE / 2;
        const vdx = vx2 - vx1, vdy = vy2 - vy1;
        const vdist = Math.sqrt(vdx * vdx + vdy * vdy);
        const vpx = -vdy / vdist, vpy = vdx / vdist;
        const vbow = vdist * 0.18;
        const vmx = (vx1 + vx2) / 2 + vpx * vbow;
        const vmy = (vy1 + vy2) / 2 + vpy * vbow;
        html += `<path d="M${vx1},${vy1} Q${vmx},${vmy} ${vx2},${vy2}" class="edge-line virtual ${vcls}"/>`;
      }
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
    } else if (step.type === 'odd_wheel') {
      html += renderOddWheelStep(step, depth);
    } else if (step.type === 'k4') {
      html += renderK4Step(step, depth);
    } else if (step.type === 'circular_ladder') {
      html += renderCircularLadderStep(step, depth);
    } else if (step.type === 'bridged_hexagon') {
      html += renderBridgedHexagonStep(step, depth);
    } else if (step.type === 'pigeonhole_xwing') {
      html += renderPigeonholeXwingStep(step, depth);
    } else if (step.type === 'set_equivalence') {
      html += renderSetEquivalenceStep(step, depth);
    } else if (step.type === 'house_coloring_contradiction') {
      html += renderHouseColoringContradictionStep(step, depth);
    } else if (step.type === 'parity_transport_deduction') {
      html += renderParityTransportDeductionStep(step, depth);
    } else if (step.type === 'trivalue_oddagon') {
      html += renderTrivalueOddagonStep(step, depth);
    } else if (step.type === 'parity_chain') {
      html += renderParityChainStep(step, depth);
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

function renderOddWheelStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'odd_wheel',
    hub: step.hub,
    rim: step.rim,
  }));

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">Odd wheel</span>: hub ` +
    `<span class="step-vertex">${esc(step.hub)}</span> forces rim to 2 colors.` +
    `<br>Bivalue oddagon ` +
    `{${step.rim.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ')}} ` +
    `(length ${step.rim.length}) cannot be 2-colored. ` +
    `<span class="step-k4">Contradiction.</span>` +
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

function renderTrivalueOddagonStep(step, depth) {
  const allCells = step.segments.flatMap(seg => seg.cells);
  const data = escapeAttr(JSON.stringify({
    type: 'trivalue_oddagon',
    cells: allCells,
    segments: step.segments,
  }));

  let html = `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">Trivalue oddagon</span>:`;

  for (const seg of step.segments) {
    const cellStr = seg.cells.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
    html += `<br>&nbsp;&nbsp;${esc(seg.house_type)} ${seg.house_id} {${cellStr}}`;
    html += `<br>&nbsp;&nbsp;&nbsp;&nbsp;\u2192 via ${esc(seg.via_type)} {${esc(seg.via_ids)}} [${esc(seg.parity)}]`;
  }

  html += `<br><span class="step-k4">Cycle parity: odd. Contradiction.</span>` +
    `</div>`;
  return html;
}

function renderParityChainStep(step, depth) {
  const allCells = step.rows.flatMap(row => row.cells);
  const data = escapeAttr(JSON.stringify({
    type: 'parity_chain',
    cells: allCells,
    rows: step.rows,
  }));

  let html = `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">Parity transport</span>:`;

  for (const row of step.rows) {
    const cellStr = row.cells.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
    html += `<br>&nbsp;&nbsp;row ${row.row_id} {${cellStr}}`;
  }

  html += `<br><span class="step-identify">${step.num_rows} same-parity permutations from 3 available \u2192 pigeonhole.</span>`;
  html += `<br><span class="step-k4">Contradiction.</span>` +
    `</div>`;
  return html;
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

function renderCircularLadderStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'circular_ladder',
    rungs: step.rungs,
    satellites: step.satellites,
  }));

  const rungStr = step.rungs.map(r => `<span class="step-vertex">${esc(r[0])}</span>\u2014<span class="step-vertex">${esc(r[1])}</span>`).join(', ');
  const satStr = step.satellites.map(([ri, name]) => `<span class="step-vertex">${esc(name)}</span> (rung ${ri + 1})`).join(', ');
  const action = step.satellites.length >= 3 ? 'Add triangle' : 'Add edge';

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-diamond">Circular ladder</span> ` +
    `{${rungStr}}` +
    `<br><span class="step-identify">Satellites ${satStr} forced to distinct colors. ${action}.</span>` +
    `</div>`;
}

function renderBridgedHexagonStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'bridged_hexagon',
    ring: step.ring,
    bridges: step.bridges,
  }));

  const ringStr = step.ring.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const bridgeStr = step.bridges.map(([s1, s2]) =>
    `<span class="step-vertex">${esc(s1)}</span>\u2014<span class="step-vertex">${esc(s2)}</span>`
  ).join(', ');

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">Bridged hexagon</span>: ring ` +
    `{${ringStr}}` +
    `<br>Bridges: ${bridgeStr}` +
    `<br><span class="step-identify">Each bridge forces opposite edges to miss different colors. Contradiction.</span>` +
    `</div>`;
}

function renderPigeonholeXwingStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'pigeonhole_xwing',
    cycle: step.cycle,
    diagonal_1: step.diagonal_1,
    diagonal_2: step.diagonal_2,
    clash_1: step.clash_1,
    clash_2: step.clash_2,
  }));

  const cycleStr = step.cycle.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const d1Str = step.diagonal_1.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const d2Str = step.diagonal_2.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const cl1Str = step.clash_1.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const cl2Str = step.clash_2.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">Pigeonhole X-wing</span> on {${cycleStr}}:` +
    `<br>Diagonals: {${d1Str}} and {${d2Str}} (non-adjacent).` +
    `<br><span class="step-identify">By pigeonhole, one diagonal must share a color.</span>` +
    `<br>Case 1: color(${esc(step.diagonal_1[0])}) = color(${esc(step.diagonal_1[1])})` +
    ` \u2192 forces ${cl1Str} (adjacent). <span class="step-k4">Contradiction.</span>` +
    `<br>Case 2: color(${esc(step.diagonal_2[0])}) = color(${esc(step.diagonal_2[1])})` +
    ` \u2192 forces ${cl2Str} (adjacent). <span class="step-k4">Contradiction.</span>` +
    `</div>`;
}

function renderSetEquivalenceStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'set_equivalence',
    lhs: step.lhs,
    rhs: step.rhs,
    is_contradiction: step.is_contradiction,
  }));

  const lhsStr = step.lhs.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');
  const rhsStr = step.rhs.map(v => `<span class="step-vertex">${esc(v)}</span>`).join(', ');

  let html = `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-set">SET</span>: ${esc(step.equation)}` +
    `<br><span class="step-identify">Remainder: {${lhsStr}} = {${rhsStr}}.</span>`;

  if (step.is_contradiction) {
    html += `<br><span class="step-k4">\u2192 ${esc(step.deduction)}</span>`;
  } else {
    html += `<br><span class="step-identify">\u2192 ${esc(step.deduction)}</span>`;
  }

  html += `</div>`;
  return html;
}

function renderHouseColoringContradictionStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'house_coloring_contradiction',
    houses: step.houses,
  }));

  const housesStr = step.houses.map(h => `<span class="step-vertex">${esc(h)}</span>`).join(', ');

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-k4">House coloring constraint</span> ` +
    `{${housesStr}}:` +
    `<br><span class="step-k4">No valid 3-coloring of these houses exists. Contradiction.</span>` +
    `</div>`;
}

function renderParityTransportDeductionStep(step, depth) {
  const data = escapeAttr(JSON.stringify({
    type: 'parity_transport_deduction',
    houses: step.houses,
    cell_a: step.cell_a,
    cell_b: step.cell_b,
    forced_same: step.forced_same,
  }));

  const housesStr = step.houses.map(h => `<span class="step-vertex">${esc(h)}</span>`).join(', ');
  const action = step.forced_same
    ? `color(${esc(step.cell_a)}) = color(${esc(step.cell_b)}). Identify.`
    : `color(${esc(step.cell_a)}) \u2260 color(${esc(step.cell_b)}). Add edge.`;

  return `<div class="proof-step" data-step='${data}'>` +
    `<span class="step-num">${step.step}.</span> ` +
    `<span class="step-diamond">House coloring constraint</span> ` +
    `{${housesStr}}:` +
    `<br><span class="step-identify">All valid 3-colorings force ${action}</span>` +
    `</div>`;
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

  // Start with accumulated diamond merge colors and virtual edges up to this step
  const proofContainer = document.getElementById('proof-view');
  const accum = computeAccumulatedColors(proofContainer, el);
  const highlights = accum.highlights;
  let edgeHL = null; // optional edge highlights (Map: 'min,max' → CSS class)
  // Seed with accumulated virtual edges from prior steps
  let virtEdges = accum.virtualEdges.length > 0 ? [...accum.virtualEdges] : null;

  if (stepData.type === 'diamond') {
    // Highlight all 4 diamond vertices (fallback for uncolored cells)
    for (const vName of stepData.vertices) {
      for (const cell of parseCellsFromLabel(vName)) {
        if (!highlights[cell]) highlights[cell] = 'highlighted';
      }
    }
    // Spine: purple (matches .step-diamond text)
    for (const cell of parseCellsFromLabel(stepData.spine_u)) {
      if (!highlights[cell] || highlights[cell] === 'highlighted')
        highlights[cell] = 'hl-diamond';
    }
    for (const cell of parseCellsFromLabel(stepData.spine_v)) {
      if (!highlights[cell] || highlights[cell] === 'highlighted')
        highlights[cell] = 'hl-diamond';
    }
    // Tips: blue (matches .step-identify text)
    for (const cell of parseCellsFromLabel(stepData.tip_a)) {
      if (!highlights[cell] || highlights[cell] === 'highlighted')
        highlights[cell] = 'hl-identify';
    }
    for (const cell of parseCellsFromLabel(stepData.tip_b)) {
      if (!highlights[cell] || highlights[cell] === 'highlighted')
        highlights[cell] = 'hl-identify';
    }
  } else if (stepData.type === 'k4') {
    // Don't recolor K₄ cells — keep accumulated/default colors.
    // Show K₄ edges in red instead.
    const k4Cells = stepData.vertices.flatMap(v => parseCellsFromLabel(v));
    edgeHL = new Map();
    for (let i = 0; i < k4Cells.length; i++) {
      for (let j = i + 1; j < k4Cells.length; j++) {
        const u = k4Cells[i], v = k4Cells[j];
        if (NEIGHBORS[u].has(v)) {
          edgeHL.set(Math.min(u, v) + ',' + Math.max(u, v), 'k4-edge');
        }
      }
    }
  } else if (stepData.type === 'odd_wheel') {
    // Hub: red (matches .step-k4 / contradiction)
    for (const cell of parseCellsFromLabel(stepData.hub)) {
      if (!highlights[cell] || highlights[cell] === 'highlighted')
        highlights[cell] = 'hl-k4';
    }
    // Rim cells: light orange (matches .step-vertex text)
    for (const vName of stepData.rim) {
      for (const cell of parseCellsFromLabel(vName)) {
        if (!highlights[cell] || highlights[cell] === 'highlighted')
          highlights[cell] = 'hl-vertex';
      }
    }
    // Compute oddagon rim edges (consecutive pairs in cycle)
    const rimCellSets = stepData.rim.map(vName => parseCellsFromLabel(vName));
    edgeHL = new Map();
    for (let ri = 0; ri < rimCellSets.length; ri++) {
      const nxt = (ri + 1) % rimCellSets.length;
      for (const u of rimCellSets[ri]) {
        for (const v of rimCellSets[nxt]) {
          if (NEIGHBORS[u].has(v)) {
            edgeHL.set(Math.min(u, v) + ',' + Math.max(u, v), 'oddagon');
          }
        }
      }
    }
  } else if (stepData.type === 'circular_ladder') {
    // Rungs: purple (matches .step-diamond text)
    for (const [a, b] of stepData.rungs) {
      for (const cell of parseCellsFromLabel(a)) highlights[cell] = 'hl-diamond';
      for (const cell of parseCellsFromLabel(b)) highlights[cell] = 'hl-diamond';
    }
    // Satellites: blue (matches .step-identify text)
    const satCells = [];
    for (const [, name] of stepData.satellites) {
      const cells = parseCellsFromLabel(name);
      for (const cell of cells) highlights[cell] = 'hl-identify';
      satCells.push(...cells);
    }
    // Virtual edges between all pairs of satellite cells
    if (!virtEdges) virtEdges = [];
    for (let i = 0; i < satCells.length; i++) {
      for (let j = i + 1; j < satCells.length; j++) {
        if (!NEIGHBORS[satCells[i]].has(satCells[j])) {
          virtEdges.push([satCells[i], satCells[j], '']);
        }
      }
    }
  } else if (stepData.type === 'bridged_hexagon') {
    // Ring: red (matches .step-k4 / contradiction)
    for (const vName of stepData.ring) {
      for (const cell of parseCellsFromLabel(vName)) highlights[cell] = 'hl-k4';
    }
    // Bridge endpoints: purple + blue (structural pair)
    for (const [s1, s2] of stepData.bridges) {
      for (const cell of parseCellsFromLabel(s1)) highlights[cell] = 'hl-diamond';
      for (const cell of parseCellsFromLabel(s2)) highlights[cell] = 'hl-identify';
    }
  } else if (stepData.type === 'pigeonhole_xwing') {
    // Cycle cells: purple (structural)
    for (const vName of stepData.cycle) {
      for (const cell of parseCellsFromLabel(vName)) highlights[cell] = 'hl-diamond';
    }
    // Clash cells: red (contradiction)
    for (const vName of stepData.clash_1) {
      for (const cell of parseCellsFromLabel(vName)) highlights[cell] = 'hl-k4';
    }
    for (const vName of stepData.clash_2) {
      for (const cell of parseCellsFromLabel(vName)) highlights[cell] = 'hl-k4';
    }
  } else if (stepData.type === 'set_equivalence') {
    // LHS: green (matches .step-set text)
    const lhsCells = [];
    for (const vName of stepData.lhs) {
      for (const cell of parseCellsFromLabel(vName)) {
        highlights[cell] = 'hl-set';
        lhsCells.push(cell);
      }
    }
    // RHS: blue (matches .step-identify text)
    const rhsCells = [];
    for (const vName of stepData.rhs) {
      for (const cell of parseCellsFromLabel(vName)) {
        highlights[cell] = 'hl-identify';
        rhsCells.push(cell);
      }
    }
    // Virtual edges within each side (for m>=2 non-contradiction)
    if (!stepData.is_contradiction && (lhsCells.length >= 2 || rhsCells.length >= 2)) {
      if (!virtEdges) virtEdges = [];
      for (const group of [lhsCells, rhsCells]) {
        for (let i = 0; i < group.length; i++) {
          for (let j = i + 1; j < group.length; j++) {
            if (!NEIGHBORS[group[i]].has(group[j])) {
              virtEdges.push([group[i], group[j], '']);
            }
          }
        }
      }
    }
  } else if (stepData.type === 'house_coloring_contradiction') {
    // Red (matches .step-k4 / contradiction)
    for (const house of stepData.houses) {
      for (const cell of parseCellsFromHouse(house)) {
        highlights[cell] = 'hl-k4';
      }
    }
  } else if (stepData.type === 'parity_transport_deduction') {
    // Houses: purple (matches .step-diamond text)
    for (const house of stepData.houses) {
      for (const cell of parseCellsFromHouse(house)) {
        highlights[cell] = 'hl-diamond';
      }
    }
    // Deduced cells: blue + green
    const cellsA = parseCellsFromLabel(stepData.cell_a);
    const cellsB = parseCellsFromLabel(stepData.cell_b);
    for (const cell of cellsA) highlights[cell] = 'hl-identify';
    for (const cell of cellsB) highlights[cell] = 'hl-set';
    // Virtual edge if forced different
    if (!stepData.forced_same) {
      if (!virtEdges) virtEdges = [];
      for (const a of cellsA) {
        for (const b of cellsB) {
          if (!NEIGHBORS[a].has(b)) {
            virtEdges.push([a, b, '']);
          }
        }
      }
    }
  } else if (stepData.type === 'trivalue_oddagon') {
    // Alternating colors per segment
    const segColors = ['hl-diamond', 'hl-identify', 'hl-set', 'hl-branch'];
    if (stepData.segments) {
      stepData.segments.forEach((seg, si) => {
        for (const vName of seg.cells) {
          for (const cell of parseCellsFromLabel(vName)) {
            highlights[cell] = segColors[si % segColors.length];
          }
        }
      });
    } else if (stepData.cells) {
      for (const vName of stepData.cells) {
        for (const cell of parseCellsFromLabel(vName)) {
          highlights[cell] = 'hl-diamond';
        }
      }
    }
  } else if (stepData.type === 'parity_chain') {
    // Alternating colors per row
    const rowColors = ['hl-diamond', 'hl-identify', 'hl-set', 'hl-branch'];
    if (stepData.rows) {
      stepData.rows.forEach((row, ri) => {
        for (const vName of row.cells) {
          for (const cell of parseCellsFromLabel(vName)) {
            highlights[cell] = rowColors[ri % rowColors.length];
          }
        }
      });
    } else if (stepData.cells) {
      for (const vName of stepData.cells) {
        for (const cell of parseCellsFromLabel(vName)) {
          highlights[cell] = 'hl-diamond';
        }
      }
    }
  } else if (stepData.type === 'branch') {
    // Two branch vertices: blue + orange
    const brA = parseCellsFromLabel(stepData.vertex_a);
    const brB = parseCellsFromLabel(stepData.vertex_b);
    for (const cell of brA) highlights[cell] = 'hl-identify';
    for (const cell of brB) highlights[cell] = 'hl-branch';
    // Virtual edge (Case B adds edge between them)
    if (!virtEdges) virtEdges = [];
    for (const a of brA) {
      for (const b of brB) {
        if (!NEIGHBORS[a].has(b)) {
          virtEdges.push([a, b, '']);
        }
      }
    }
  }

  renderGrid(pattern, highlights, edgeHL, virtEdges);
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

// Parse house name like "row 6", "col 3", "box 9" into cell indices
function parseCellsFromHouse(house) {
  const cells = [];
  const rm = house.match(/^row (\d+)$/);
  if (rm) {
    const r = parseInt(rm[1]) - 1;
    for (let c = 0; c < 9; c++) cells.push(r * 9 + c);
    return cells;
  }
  const cm = house.match(/^col (\d+)$/);
  if (cm) {
    const c = parseInt(cm[1]) - 1;
    for (let r = 0; r < 9; r++) cells.push(r * 9 + c);
    return cells;
  }
  const bm = house.match(/^box (\d+)$/);
  if (bm) {
    const b = parseInt(bm[1]) - 1;
    const br = 3 * Math.floor(b / 3), bc = 3 * (b % 3);
    for (let dr = 0; dr < 3; dr++)
      for (let dc = 0; dc < 3; dc++)
        cells.push((br + dr) * 9 + (bc + dc));
    return cells;
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

  // Difficulty filter buttons
  for (const btn of document.querySelectorAll('#diff-buttons .btn')) {
    btn.addEventListener('click', () => {
      document.querySelector('#diff-buttons .btn.active')?.classList.remove('active');
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

  // Copy ASCII grid
  document.getElementById('copy-ascii').addEventListener('click', () => {
    copyAsciiGrid();
  });

  // Export filtered patterns as text file
  document.getElementById('export-filtered').addEventListener('click', () => {
    exportFilteredPatterns();
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
      } else if (s.type === 'odd_wheel') {
        lines.push(`${indent}${s.step}. Odd wheel: hub ${s.hub} forces rim to 2 colors.`);
        lines.push(`${indent}   Bivalue oddagon {${s.rim.join(', ')}} (length ${s.rim.length}) cannot be 2-colored. Contradiction.`);
      } else if (s.type === 'circular_ladder') {
        const rungStr = s.rungs.map(r => `${r[0]}\u2014${r[1]}`).join(', ');
        const satStr = s.satellites.map(([ri, name]) => `${name} (rung ${ri + 1})`).join(', ');
        const action = s.satellites.length >= 3 ? 'Add triangle' : 'Add edge';
        lines.push(`${indent}${s.step}. Circular ladder {${rungStr}}`);
        lines.push(`${indent}   Satellites ${satStr} forced to distinct colors. ${action}.`);
      } else if (s.type === 'bridged_hexagon') {
        lines.push(`${indent}${s.step}. Bridged hexagon: ring {${s.ring.join(', ')}}`);
        const bridgeStr = s.bridges.map(([s1, s2]) => `${s1}\u2014${s2}`).join(', ');
        lines.push(`${indent}   Bridges: ${bridgeStr}`);
        lines.push(`${indent}   Each bridge forces opposite edges to miss different colors. Contradiction.`);
      } else if (s.type === 'pigeonhole_xwing') {
        lines.push(`${indent}${s.step}. Pigeonhole X-wing on {${s.cycle.join(', ')}}:`);
        lines.push(`${indent}   Diagonals: {${s.diagonal_1.join(', ')}} and {${s.diagonal_2.join(', ')}} (non-adjacent).`);
        lines.push(`${indent}   By pigeonhole, one diagonal must share a color.`);
        lines.push(`${indent}   Case 1: color(${s.diagonal_1[0]}) = color(${s.diagonal_1[1]}) \u2192 forces ${s.clash_1.join(' = ')} (adjacent). Contradiction.`);
        lines.push(`${indent}   Case 2: color(${s.diagonal_2[0]}) = color(${s.diagonal_2[1]}) \u2192 forces ${s.clash_2.join(' = ')} (adjacent). Contradiction.`);
      } else if (s.type === 'set_equivalence') {
        lines.push(`${indent}${s.step}. SET: ${s.equation}`);
        lines.push(`${indent}   Remainder: {${s.lhs.join(', ')}} = {${s.rhs.join(', ')}}.`);
        lines.push(`${indent}   \u2192 ${s.deduction}`);
      } else if (s.type === 'trivalue_oddagon') {
        lines.push(`${indent}${s.step}. Trivalue oddagon:`);
        for (const seg of s.segments) {
          lines.push(`${indent}   ${seg.house_type} ${seg.house_id} {${seg.cells.join(', ')}}`);
          lines.push(`${indent}     \u2192 via ${seg.via_type} {${seg.via_ids}} [${seg.parity}]`);
        }
        lines.push(`${indent}   Cycle parity: odd. Contradiction.`);
      } else if (s.type === 'parity_chain') {
        lines.push(`${indent}${s.step}. Parity transport:`);
        for (const row of s.rows) {
          lines.push(`${indent}   row ${row.row_id} {${row.cells.join(', ')}}`);
        }
        lines.push(`${indent}   ${s.num_rows} same-parity permutations from 3 available \u2192 pigeonhole contradiction.`);
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
    .cell-active { fill: #484f58; opacity: 0.7; }
    .cell-active.highlighted { fill: #8b949e; opacity: 0.85; }
    .cell-active.hl-diamond { fill: #a371f7; opacity: 0.85; }
    .cell-active.hl-identify { fill: #79c0ff; opacity: 0.85; }
    .cell-active.hl-k4 { fill: #f85149; opacity: 0.95; }
    .cell-active.hl-set { fill: #7ee787; opacity: 0.85; }
    .cell-active.hl-branch { fill: #f0883e; opacity: 0.85; }
    .cell-active.hl-vertex { fill: #ffa657; opacity: 0.85; }
    .cell-active.color-a { fill: #3fb950; opacity: 0.85; }
    .cell-active.color-b { fill: #d4a017; opacity: 0.85; }
    .cell-active.color-c { fill: #58a6ff; opacity: 0.85; }
    .cell-active.color-d { fill: #ffa657; opacity: 0.85; }
    .grid-line-thin { stroke: #30363d; stroke-width: 0.5; }
    .grid-line-thick { stroke: #484f58; stroke-width: 2; }
    .grid-border { stroke: #6e7681; stroke-width: 2.5; fill: none; }
    .edge-line { stroke: #484f58; stroke-width: 0.6; opacity: 0.4; fill: none; }
    .edge-line.highlighted { stroke: #8b949e; stroke-width: 1.2; opacity: 0.7; }
    .edge-line.oddagon { stroke: #39d353; stroke-width: 2; opacity: 0.9; }
    .edge-line.k4-edge { stroke: #f85149; stroke-width: 2.5; opacity: 0.95; }
    .edge-line.virtual { stroke: #8b949e; stroke-width: 1.5; stroke-dasharray: 4 3; opacity: 0.85; }
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

// ── Export filtered patterns ─────────────────────────────────────
function exportFilteredPatterns() {
  if (!filteredPatterns.length) {
    showCopyToast('No patterns to export');
    return;
  }
  const text = filteredPatterns.map(p => p.bitstring).join('\n') + '\n';
  const blob = new Blob([text], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `patterns_${filteredPatterns.length}.txt`;
  a.click();
  URL.revokeObjectURL(url);
  showCopyToast(`Exported ${filteredPatterns.length} patterns`);
}

// ── ASCII grid export ───────────────────────────────────────────
function copyAsciiGrid() {
  if (!selectedPattern) return;
  const cellSet = new Set(selectedPattern.cell_indices);
  const lines = [];
  lines.push(`${selectedPattern.id}`);
  lines.push('+-------+-------+-------+');
  for (let r = 0; r < 9; r++) {
    let row = '|';
    for (let c = 0; c < 9; c++) {
      const idx = r * 9 + c;
      row += ' ' + (cellSet.has(idx) ? 'X' : '.');
      if (c % 3 === 2) row += ' |';
    }
    lines.push(row);
    if (r % 3 === 2) lines.push('+-------+-------+-------+');
  }
  const text = lines.join('\n');
  navigator.clipboard.writeText(text)
    .then(() => showCopyToast('ASCII grid copied'))
    .catch(() => { fallbackCopy(text); showCopyToast('ASCII grid copied'); });
}

// ── Start ───────────────────────────────────────────────────────
init();
