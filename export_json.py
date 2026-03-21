"""Export pattern and proof data to JSON for the web viewer."""

import json
import re
import sys
from pathlib import Path

# Sudoku graph adjacency
def make_sudoku_neighbors():
    neighbors = [set() for _ in range(81)]
    for v in range(81):
        r, c = divmod(v, 9)
        br, bc = 3 * (r // 3), 3 * (c // 3)
        for u in range(81):
            if u == v:
                continue
            r2, c2 = divmod(u, 9)
            if r2 == r or c2 == c or (3 * (r2 // 3) == br and 3 * (c2 // 3) == bc):
                neighbors[v].add(u)
    return neighbors

NEIGHBORS = make_sudoku_neighbors()


def parse_bitstring(s):
    """Parse 81-char bitstring to list of cell indices."""
    return [i for i, ch in enumerate(s.strip()) if ch == '1']


def compute_properties(cells):
    """Compute graph properties for a pattern."""
    cell_set = set(cells)
    edges = []
    degrees = {c: 0 for c in cells}
    for i, u in enumerate(cells):
        for j in range(i + 1, len(cells)):
            v = cells[j]
            if v in NEIGHBORS[u]:
                edges.append([i, j])
                degrees[u] += 1
                degrees[v] += 1
    deg_seq = sorted([degrees[c] for c in cells], reverse=True)
    return edges, deg_seq


def translate_cell_name(name, iso):
    """Translate cell references like 'r1c4' or '[r1c2=r3c5]' using an isomorphism mapping."""
    def repl(m):
        r, c = int(m.group(1)) - 1, int(m.group(2)) - 1
        cell = r * 9 + c
        mapped = iso.get(cell, cell)
        mr, mc = divmod(mapped, 9)
        return f'r{mr+1}c{mc+1}'
    return re.sub(r'r(\d+)c(\d+)', repl, name)


def translate_proof_tree(tree, iso):
    """Recursively translate all cell references in a proof tree to minlex form."""
    result = []
    for step in tree:
        s = dict(step)
        if s['type'] == 'diamond':
            s['vertices'] = [translate_cell_name(v, iso) for v in s['vertices']]
            s['tip_a'] = translate_cell_name(s['tip_a'], iso)
            s['tip_b'] = translate_cell_name(s['tip_b'], iso)
            s['spine_u'] = translate_cell_name(s['spine_u'], iso)
            s['spine_v'] = translate_cell_name(s['spine_v'], iso)
        elif s['type'] == 'k4':
            s['vertices'] = [translate_cell_name(v, iso) for v in s['vertices']]
        elif s['type'] == 'odd_wheel':
            s['hub'] = translate_cell_name(s['hub'], iso)
            s['rim'] = [translate_cell_name(v, iso) for v in s['rim']]
        elif s['type'] == 'circular_ladder':
            s['rungs'] = [[translate_cell_name(a, iso), translate_cell_name(b, iso)] for a, b in s['rungs']]
            s['satellites'] = [[ri, translate_cell_name(name, iso)] for ri, name in s['satellites']]
        elif s['type'] == 'bridged_hexagon':
            s['ring'] = [translate_cell_name(v, iso) for v in s['ring']]
            s['bridges'] = [[translate_cell_name(a, iso), translate_cell_name(b, iso)] for a, b in s['bridges']]
        elif s['type'] == 'set_equivalence':
            s['lhs'] = [translate_cell_name(v, iso) for v in s['lhs']]
            s['rhs'] = [translate_cell_name(v, iso) for v in s['rhs']]
        elif s['type'] == 'parity_transport_deduction':
            s['cell_a'] = translate_cell_name(s['cell_a'], iso)
            s['cell_b'] = translate_cell_name(s['cell_b'], iso)
        elif s['type'] == 'trivalue_oddagon':
            s['segments'] = [
                {**seg, 'cells': [translate_cell_name(c, iso) for c in seg['cells']]}
                for seg in s['segments']
            ]
        elif s['type'] == 'parity_chain':
            s['rows'] = [
                {**row, 'cells': [translate_cell_name(c, iso) for c in row['cells']]}
                for row in s['rows']
            ]
        elif s['type'] == 'branch':
            s['vertex_a'] = translate_cell_name(s['vertex_a'], iso)
            s['vertex_b'] = translate_cell_name(s['vertex_b'], iso)
            s['case_a'] = translate_proof_tree(s['case_a'], iso)
            s['case_b'] = translate_proof_tree(s['case_b'], iso)
        # house_coloring_contradiction has no cell refs to translate
        result.append(s)
    return result


def find_isomorphism(cells_a, cells_b):
    """Find a bijection from cells_a to cells_b preserving sudoku-graph adjacency.
    Returns dict {cell_a: cell_b} or None."""
    n = len(cells_a)
    if n != len(cells_b):
        return None

    # Build adjacency sets within each cell list
    adj_a = {u: set() for u in cells_a}
    adj_b = {u: set() for u in cells_b}
    for u in cells_a:
        for v in cells_a:
            if v != u and v in NEIGHBORS[u]:
                adj_a[u].add(v)
    for u in cells_b:
        for v in cells_b:
            if v != u and v in NEIGHBORS[u]:
                adj_b[u].add(v)

    deg_a = {u: len(adj_a[u]) for u in cells_a}
    deg_b = {u: len(adj_b[u]) for u in cells_b}

    # Order source vertices by descending degree (most constrained first)
    sorted_a = sorted(cells_a, key=lambda v: -deg_a[v])
    # Precompute candidates by degree
    cands = {u: [v for v in cells_b if deg_b[v] == deg_a[u]] for u in sorted_a}

    mapping = {}
    used = set()

    def backtrack(idx):
        if idx == n:
            return True
        u = sorted_a[idx]
        for v in cands[u]:
            if v in used:
                continue
            # Check adjacency consistency with already-mapped vertices
            ok = True
            for prev_u, prev_v in mapping.items():
                if (prev_u in adj_a[u]) != (prev_v in adj_b[v]):
                    ok = False
                    break
            if not ok:
                continue
            mapping[u] = v
            used.add(v)
            if backtrack(idx + 1):
                return True
            del mapping[u]
            used.discard(v)
        return False

    if backtrack(0):
        return mapping
    return None


def parse_proof_file(path):
    """Parse a proof text file into structured proof data per pattern."""
    text = Path(path).read_text(encoding='utf-8')
    # Split into pattern blocks
    blocks = re.split(r'(?=^pattern \d+/\d+:)', text, flags=re.MULTILINE)
    results = []
    for block in blocks:
        block = block.strip()
        if not block:
            continue
        # Parse header — try formats from newest to oldest
        fmt = None
        header_match = re.match(
            r'pattern (\d+)/(\d+): (PROVED|FAILED) cells=(\d+) '
            r'depth=(\d+) diamonds=(\d+) odd_wheels=(\d+) circular_ladders=(\d+) bridged_hexagons=(\d+) set_equivalences=(\d+) parity_transports=(\d+) branches=(\d+) complete=(\w+)'
            r'(?: greedy_branches=(\d+) greedy_odd_wheels=(\d+) greedy_circular_ladders=(\d+) greedy_bridged_hexagons=(\d+) greedy_set_equivalences=(\d+) greedy_parity_transports=(\d+))?',
            block
        )
        if header_match:
            fmt = 'new'
        if not header_match:
            header_match = re.match(
                r'pattern (\d+)/(\d+): (PROVED|FAILED) cells=(\d+) '
                r'depth=(\d+) diamonds=(\d+) odd_wheels=(\d+) circular_ladders=(\d+) bridged_hexagons=(\d+) set_equivalences=(\d+) branches=(\d+) complete=(\w+)'
                r'(?: greedy_branches=(\d+) greedy_odd_wheels=(\d+) greedy_circular_ladders=(\d+) greedy_bridged_hexagons=(\d+) greedy_set_equivalences=(\d+))?',
                block
            )
            if header_match:
                fmt = 'mid'
        if not header_match:
            header_match = re.match(
                r'pattern (\d+)/(\d+): (PROVED|FAILED) cells=(\d+) '
                r'depth=(\d+) diamonds=(\d+) odd_wheels=(\d+) circular_ladders=(\d+) bridged_hexagons=(\d+) branches=(\d+) complete=(\w+)'
                r'(?: greedy_branches=(\d+) greedy_odd_wheels=(\d+) greedy_circular_ladders=(\d+) greedy_bridged_hexagons=(\d+))?',
                block
            )
            if header_match:
                fmt = 'old'
        if not header_match:
            continue

        idx = int(header_match.group(1))
        status = header_match.group(3)
        cells_count = int(header_match.group(4))
        depth = int(header_match.group(5))
        diamonds = int(header_match.group(6))
        odd_wheels = int(header_match.group(7))
        circular_ladders = int(header_match.group(8))
        bridged_hexagons = int(header_match.group(9))

        if fmt == 'new':
            set_equivalences = int(header_match.group(10))
            parity_transports = int(header_match.group(11))
            branches = int(header_match.group(12))
            complete = header_match.group(13) == 'true'
            greedy_branches = int(header_match.group(14)) if header_match.group(14) else branches
            greedy_odd_wheels = int(header_match.group(15)) if header_match.group(15) else odd_wheels
            greedy_circular_ladders = int(header_match.group(16)) if header_match.group(16) else circular_ladders
            greedy_bridged_hexagons = int(header_match.group(17)) if header_match.group(17) else bridged_hexagons
            greedy_set_equivalences = int(header_match.group(18)) if header_match.group(18) else set_equivalences
            greedy_parity_transports = int(header_match.group(19)) if header_match.group(19) else parity_transports
        elif fmt == 'mid':
            set_equivalences = int(header_match.group(10))
            parity_transports = 0
            branches = int(header_match.group(11))
            complete = header_match.group(12) == 'true'
            greedy_branches = int(header_match.group(13)) if header_match.group(13) else branches
            greedy_odd_wheels = int(header_match.group(14)) if header_match.group(14) else odd_wheels
            greedy_circular_ladders = int(header_match.group(15)) if header_match.group(15) else circular_ladders
            greedy_bridged_hexagons = int(header_match.group(16)) if header_match.group(16) else bridged_hexagons
            greedy_set_equivalences = int(header_match.group(17)) if header_match.group(17) else set_equivalences
            greedy_parity_transports = 0
        else:  # old
            set_equivalences = 0
            parity_transports = 0
            branches = int(header_match.group(10))
            complete = header_match.group(11) == 'true'
            greedy_branches = int(header_match.group(12)) if header_match.group(12) else branches
            greedy_odd_wheels = int(header_match.group(13)) if header_match.group(13) else odd_wheels
            greedy_circular_ladders = int(header_match.group(14)) if header_match.group(14) else circular_ladders
            greedy_bridged_hexagons = int(header_match.group(15)) if header_match.group(15) else bridged_hexagons
            greedy_set_equivalences = 0
            greedy_parity_transports = 0

        lines = block.split('\n')
        bitstring = lines[1].strip() if len(lines) > 1 else ''

        # Extract proof text (everything after the bitstring)
        proof_text = '\n'.join(lines[2:]).strip() if len(lines) > 2 else ''

        # Parse structured proof steps
        proof_steps = parse_proof_steps(lines[2:])

        results.append({
            'index': idx,
            'bitstring': bitstring,
            'status': status,
            'cells_count': cells_count,
            'depth': depth,
            'diamonds': diamonds,
            'odd_wheels': odd_wheels,
            'circular_ladders': circular_ladders,
            'bridged_hexagons': bridged_hexagons,
            'set_equivalences': set_equivalences,
            'parity_transports': parity_transports,
            'branches': branches,
            'complete': complete,
            'greedy_branches': greedy_branches,
            'greedy_odd_wheels': greedy_odd_wheels,
            'greedy_circular_ladders': greedy_circular_ladders,
            'greedy_bridged_hexagons': greedy_bridged_hexagons,
            'greedy_set_equivalences': greedy_set_equivalences,
            'greedy_parity_transports': greedy_parity_transports,
            'proof_text': proof_text,
            'proof_tree': proof_steps,
        })
    return results


def parse_proof_steps(lines):
    """Parse proof text lines into a structured tree."""
    steps = []
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # Skip empty lines and preamble
        if not stripped or stripped.startswith('Proof of') or stripped.startswith('Assume') or stripped.startswith('Therefore'):
            i += 1
            continue

        # Diamond step
        dm = re.match(r'(\d+)\.\s+Diamond \{(.+?)\} \(spine (.+?)—(.+?)\)\.', stripped)
        if dm:
            step_num = int(dm.group(1))
            verts = [v.strip() for v in dm.group(2).split(',')]
            spine_u = dm.group(3)
            spine_v = dm.group(4)
            i += 1
            id_line = lines[i].strip() if i < len(lines) else ''
            id_match = re.match(r'→ color\((.+?)\) = color\((.+?)\)\. Identify\.', id_line)
            tip_a = id_match.group(1) if id_match else verts[0]
            tip_b = id_match.group(2) if id_match else verts[-1]
            steps.append({
                'type': 'diamond',
                'step': step_num,
                'tip_a': tip_a,
                'tip_b': tip_b,
                'spine_u': spine_u,
                'spine_v': spine_v,
                'vertices': verts,
            })
            i += 1
            continue

        # K4 contradiction
        km = re.match(r'(\d+)\.\s+K₄ on \{(.+?)\}\. Contradiction\.', stripped)
        if km:
            step_num = int(km.group(1))
            verts = [v.strip() for v in km.group(2).split(',')]
            steps.append({
                'type': 'k4',
                'step': step_num,
                'vertices': verts,
            })
            i += 1
            continue

        # Odd wheel contradiction
        wm = re.match(r'(\d+)\.\s+Odd wheel: hub (.+?) forces rim to 2 colors\.', stripped)
        if wm:
            step_num = int(wm.group(1))
            hub = wm.group(2)
            # Next line has the bivalue oddagon
            i += 1
            rim_line = lines[i].strip() if i < len(lines) else ''
            rim_match = re.match(r'Bivalue oddagon \{(.+?)\} \(length (\d+)\)', rim_line)
            rim = [v.strip() for v in rim_match.group(1).split(',')] if rim_match else []
            steps.append({
                'type': 'odd_wheel',
                'step': step_num,
                'hub': hub,
                'rim': rim,
            })
            i += 1
            continue

        # Circular ladder step
        clm = re.match(r'(\d+)\.\s+Circular ladder \{(.+?)\}:', stripped)
        if clm:
            step_num = int(clm.group(1))
            rung_strs = clm.group(2).split(', ')
            rungs = []
            for rs in rung_strs:
                parts = re.split(r'\u2014', rs)  # em-dash
                rungs.append(parts)
            i += 1
            sat_line = lines[i].strip() if i < len(lines) else ''
            sat_match = re.match(r'Satellites (.+?) forced to distinct colors\. .+\.', sat_line)
            satellites = []
            if sat_match:
                sat_str = sat_match.group(1)
                for sm in re.finditer(r'(\S+) \(rung (\d+)\)', sat_str):
                    satellites.append([int(sm.group(2)) - 1, sm.group(1)])
            steps.append({
                'type': 'circular_ladder',
                'step': step_num,
                'rungs': rungs,
                'satellites': satellites,
            })
            i += 1
            continue

        # Bridged hexagon contradiction
        bhm = re.match(r'(\d+)\.\s+Bridged hexagon: ring \{(.+?)\}', stripped)
        if bhm:
            step_num = int(bhm.group(1))
            ring = [v.strip() for v in bhm.group(2).split(',')]
            # Next line: Bridges: s1—s2 (edges ...), ...
            i += 1
            bridge_line = lines[i].strip() if i < len(lines) else ''
            bridges = []
            for bm2 in re.finditer(r'(\S+)\u2014(\S+) \(edges', bridge_line):
                bridges.append([bm2.group(1), bm2.group(2)])
            # Skip the two explanation lines
            i += 1
            if i < len(lines):
                i += 1
            steps.append({
                'type': 'bridged_hexagon',
                'step': step_num,
                'ring': ring,
                'bridges': bridges,
            })
            i += 1
            continue

        # SET equivalence step
        sm = re.match(r'(\d+)\.\s+SET: (.+)\.', stripped)
        if sm:
            step_num = int(sm.group(1))
            equation = sm.group(2)
            i += 1
            # Next line: Remainder: {lhs} = {rhs}.
            remainder_line = lines[i].strip() if i < len(lines) else ''
            lhs_cells = []
            rhs_cells = []
            rem_match = re.match(r'Remainder: \{(.+?)\} = \{(.+?)\}\.', remainder_line)
            if rem_match:
                lhs_cells = [v.strip() for v in rem_match.group(1).split(',')]
                rhs_cells = [v.strip() for v in rem_match.group(2).split(',')]
            i += 1
            # Next line: → deduction text
            ded_line = lines[i].strip() if i < len(lines) else ''
            deduction = ''
            if ded_line.startswith('\u2192 ') or ded_line.startswith('→ '):
                deduction = ded_line[2:]
            is_contradiction = 'Contradiction' in deduction
            steps.append({
                'type': 'set_equivalence',
                'step': step_num,
                'equation': equation,
                'lhs': lhs_cells,
                'rhs': rhs_cells,
                'deduction': deduction,
                'is_contradiction': is_contradiction,
            })
            i += 1
            continue

        # Trivalue oddagon contradiction
        tom = re.match(r'(\d+)\.\s+Trivalue oddagon:', stripped)
        if tom:
            step_num = int(tom.group(1))
            segments = []
            i += 1
            while i < len(lines):
                sl = lines[i].strip()
                if not sl:
                    i += 1
                    continue
                # Segment: "box 5 {r4c6, r5c5, r6c4}" or "row 3 {r3c3, r3c6, r3c8}"
                seg_m = re.match(r'(box|row|col)\s+(\d+)\s+\{(.+?)\}', sl)
                if seg_m:
                    seg_type = seg_m.group(1)
                    seg_id = int(seg_m.group(2))
                    seg_cells = [v.strip() for v in seg_m.group(3).split(',')]
                    # Next line: "→ via cols {4, 5, 6} [even]"
                    i += 1
                    via_line = lines[i].strip() if i < len(lines) else ''
                    via_m = re.match(r'→ via (cols|rows)\s+\{(.+?)\}\s+\[(\w+)\]', via_line)
                    via_type = via_m.group(1) if via_m else ''
                    via_ids = via_m.group(2) if via_m else ''
                    parity = via_m.group(3) if via_m else ''
                    segments.append({
                        'house_type': seg_type,
                        'house_id': seg_id,
                        'cells': seg_cells,
                        'via_type': via_type,
                        'via_ids': via_ids,
                        'parity': parity,
                    })
                    i += 1
                    continue
                # "Cycle parity: odd. Contradiction."
                if sl.startswith('Cycle parity'):
                    i += 1
                    break
                break
            steps.append({
                'type': 'trivalue_oddagon',
                'step': step_num,
                'segments': segments,
            })
            continue

        # Parity chain (parity transport) contradiction
        pcm = re.match(r'(\d+)\.\s+Parity transport:', stripped)
        if pcm:
            step_num = int(pcm.group(1))
            rows = []
            links_text = ''
            i += 1
            while i < len(lines):
                sl = lines[i].strip()
                if not sl:
                    i += 1
                    continue
                # Row: "row 5 {r5c3, r5c6, r5c9}"
                row_m = re.match(r'row\s+(\d+)\s+\{(.+?)\}', sl)
                if row_m:
                    row_id = int(row_m.group(1))
                    row_cells = [v.strip() for v in row_m.group(2).split(',')]
                    rows.append({
                        'row_id': row_id,
                        'cells': row_cells,
                    })
                    i += 1
                    continue
                # "Parallel links: ..."
                if sl.startswith('Parallel links:'):
                    links_text = sl
                    i += 1
                    continue
                # "→ same permutation parity." or "pigeonhole contradiction."
                if 'pigeonhole contradiction' in sl:
                    i += 1
                    break
                i += 1
            steps.append({
                'type': 'parity_chain',
                'step': step_num,
                'rows': rows,
                'links': links_text,
                'num_rows': len(rows),
            })
            continue

        # House coloring contradiction
        hcm = re.match(r'(\d+)\.\s+House coloring constraint \(\{(.+?)\}\):', stripped)
        if hcm:
            step_num = int(hcm.group(1))
            houses = [h.strip() for h in hcm.group(2).split(',')]
            i += 1
            next_line = lines[i].strip() if i < len(lines) else ''
            is_contradiction = 'No valid 3-coloring' in next_line
            if is_contradiction:
                steps.append({
                    'type': 'house_coloring_contradiction',
                    'step': step_num,
                    'houses': houses,
                })
            else:
                # Parity transport deduction: "All valid 3-colorings force color(A) = color(B). Identify."
                cell_a = cell_b = ''
                forced_same = True
                fm = re.search(r'color\((.+?)\)\s*=\s*color\((.+?)\)', next_line)
                if fm:
                    cell_a, cell_b = fm.group(1), fm.group(2)
                    forced_same = True
                else:
                    fm = re.search(r'color\((.+?)\)\s*≠\s*color\((.+?)\)', next_line)
                    if fm:
                        cell_a, cell_b = fm.group(1), fm.group(2)
                        forced_same = False
                steps.append({
                    'type': 'parity_transport_deduction',
                    'step': step_num,
                    'houses': houses,
                    'cell_a': cell_a,
                    'cell_b': cell_b,
                    'forced_same': forced_same,
                })
            i += 1
            continue

        # Branch step
        bm = re.match(r'(\d+)\.\s+Branch on (.+?), (.+?):', stripped)
        if bm:
            step_num = int(bm.group(1))
            vertex_a = bm.group(2)
            vertex_b = bm.group(3)
            branch_indent = len(line) - len(line.lstrip())
            # Case markers should appear at branch_indent + 4
            case_marker_indent = branch_indent + 4
            i += 1

            case_a_lines = []
            case_b_lines = []
            current_case = None

            while i < len(lines):
                cl = lines[i]
                cs = cl.strip()

                if not cs:
                    i += 1
                    continue

                cur_indent = len(cl) - len(cl.lstrip())

                # Only detect Case A/B and "Both cases" at the expected indent
                if cur_indent == case_marker_indent:
                    if re.match(r'Case A:', cs):
                        current_case = 'A'
                        i += 1
                        continue
                    elif re.match(r'Case B:', cs):
                        current_case = 'B'
                        i += 1
                        continue
                    elif cs.startswith('Both cases contradict'):
                        i += 1
                        break

                # De-indented back to or beyond branch level = done
                if cur_indent <= branch_indent and not cs.startswith('Case') and not cs.startswith('Both'):
                    break

                if current_case == 'A':
                    case_a_lines.append(cl)
                elif current_case == 'B':
                    case_b_lines.append(cl)
                i += 1

            steps.append({
                'type': 'branch',
                'step': step_num,
                'vertex_a': vertex_a,
                'vertex_b': vertex_b,
                'case_a': parse_proof_steps(case_a_lines),
                'case_b': parse_proof_steps(case_b_lines),
            })
            continue

        i += 1

    return steps


def main():
    base = Path(__file__).parent
    sizes = [10, 12, 13, 14, 15, 16]
    all_patterns = []

    for n in sizes:
        proof_file = base / f'proofs_n{n}.txt'
        minlex_file = base / f'results_n{n}_minlex_ordered.txt'

        if not proof_file.exists():
            print(f"Warning: {proof_file} not found, skipping N={n}")
            continue

        proofs = parse_proof_file(proof_file)

        # Read ordered minlex bitstrings (same order as results/proofs)
        minlex_strings = []
        if minlex_file.exists():
            minlex_strings = [l.strip() for l in minlex_file.read_text().splitlines() if l.strip()]
        if len(minlex_strings) != len(proofs):
            print(f"Warning: N={n} minlex count ({len(minlex_strings)}) != proof count ({len(proofs)})")
            minlex_strings = [''] * len(proofs)

        print(f"N={n}: parsed {len(proofs)} proofs")

        for i, proof_data in enumerate(proofs):
            bs = proof_data['bitstring']
            minlex_bs = minlex_strings[i]
            cells = parse_bitstring(bs)
            minlex_cells = parse_bitstring(minlex_bs) if minlex_bs else cells
            edges, deg_seq = compute_properties(cells)
            minlex_edges, _ = compute_properties(minlex_cells)
            cell_coords = [[c // 9, c % 9] for c in cells]
            minlex_cell_coords = [[c // 9, c % 9] for c in minlex_cells]

            # Compute proof-cell → minlex-cell mapping via graph isomorphism
            iso = find_isomorphism(cells, minlex_cells) if cells != minlex_cells else {c: c for c in cells}
            if iso is None:
                print(f"Warning: N={n} pattern {i+1}: isomorphism not found!")
                iso = {}

            # Translate proof tree cell references to minlex form
            translated_tree = translate_proof_tree(proof_data['proof_tree'], iso)

            # Determine which rows/bands are occupied (from minlex form)
            rows_used = sorted(set(c // 9 for c in minlex_cells))

            pattern = {
                'id': f'N{n}_{i+1:04d}',
                'size': n,
                'bitstring': minlex_bs,
                'cells': minlex_cell_coords,
                'cell_indices': minlex_cells,
                'edges': minlex_edges,
                'num_edges': len(edges),
                'degree_sequence': deg_seq,
                'min_degree': min(deg_seq) if deg_seq else 0,
                'max_degree': max(deg_seq) if deg_seq else 0,
                'rows_used': rows_used,
                'proof': {
                    'depth': proof_data['depth'],
                    'diamonds': proof_data['diamonds'],
                    'odd_wheels': proof_data['odd_wheels'],
                    'circular_ladders': proof_data['circular_ladders'],
                    'bridged_hexagons': proof_data['bridged_hexagons'],
                    'set_equivalences': proof_data['set_equivalences'],
                    'parity_transports': proof_data['parity_transports'],
                    'branches': proof_data['branches'],
                    'complete': proof_data['complete'],
                    'greedy_branches': proof_data['greedy_branches'],
                    'greedy_odd_wheels': proof_data['greedy_odd_wheels'],
                    'greedy_circular_ladders': proof_data['greedy_circular_ladders'],
                    'greedy_bridged_hexagons': proof_data['greedy_bridged_hexagons'],
                    'greedy_set_equivalences': proof_data['greedy_set_equivalences'],
                    'greedy_parity_transports': proof_data['greedy_parity_transports'],
                    'tree': translated_tree,
                },
            }

            all_patterns.append(pattern)

    output = {
        'generated': '2025',
        'total_patterns': len(all_patterns),
        'sizes': {str(n): sum(1 for p in all_patterns if p['size'] == n) for n in sizes},
        'patterns': all_patterns,
    }

    out_path = base / 'web' / 'data' / 'patterns.json'
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(output, separators=(',', ':')), encoding='utf-8')
    print(f"Exported {len(all_patterns)} patterns to {out_path}")
    print(f"File size: {out_path.stat().st_size / 1024:.1f} KB")


if __name__ == '__main__':
    main()
