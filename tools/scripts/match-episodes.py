#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 TD2192-Modern contributors
"""Cross-episode function matching (M1-09): file0001.exe (E1) -> td2.exe / td3.exe.

Inputs (read-only):
  * the per-instruction tables written by tools/ghidra/ExportInstructionTable.java
    (<exports>/<exe>-insns.tsv, <exe>-funcs.tsv; they hold original bytes, so they live outside
    the repo), and
  * the original executables (--data), for relocations, Pascal strings and raw bytes.
Outputs (to --out, outside the repo): match tables with addresses, counts and our classification.
Nothing here embeds game data.

BP7 compiles each unit into one code segment, so matching runs unit by unit (the segment
correspondence is EXECUTABLES.md section 1.1). Passes, in order:

  offset     same-size unit: same offset, same instruction skeleton (works even where Ghidra has
             no function at the E2/E3 address)
  exact      same instruction stream with address operands ignored, unique in the unit
  shape      same opcode/register skeleton (Ghidra instruction mask), unique in the unit
  search     E1 body found verbatim (skeleton) at exactly one place in the target unit
  callgraph  n-th callee of a matched pair, both unmatched and similar (>= 0.5)
  order      unmatched functions between two matched anchors, in address order, if similar
  best       best remaining similarity in the unit (>= 0.70)

Each pair is then compared by walking the E1 instructions over the target image:

  IDENT    every instruction equal, or differing only in operands that are addresses which
           correspond: relocated segment words, call/jump targets that map to each other, Pascal
           string literals with the same text, and data offsets that follow the DS offset map
           (learned by voting over all pairs, then accepted per offset or by neighbour delta)
  CONST    same skeleton but some operands differ for no address reason (listed in constdiff)
  CHANGED  skeleton differs; sim = difflib ratio of the skeleton token streams

Usage:
  python tools/scripts/match-episodes.py --exports P:/msdos/TD2192/re-work/exports/re7 \
      --out P:/msdos/TD2192/re-work/exports/re7/match
"""
import argparse
import bisect
import collections
import csv
import difflib
import hashlib
import os
import struct
import sys

# Code segments per exe, in image order (EXECUTABLES.md 1.1). The last entry is DGROUP.
SEGS = {
    'file0001': ['0000', '051A', '0E8B', '1E2A', '1EBF', '1EDE', '2404', '2A15', '2C37', '3292', '34E6',
                 '386E', '3A1C', '3A7E', '3A92', '3A9A', '3C43'],
    'td2': ['0000', '04EC', '0E96', '1D51', '1DE6', '1E0B', '2236', '2555', '26CB', '2D26', '2F63',
            '32EB', '3499', '34FB', '3505', '350D', '36B6'],
    'td3': ['0000', '04A0', '0C5D', '1B1E', '1BB3', '1BD8', '1FEB', '2215', '22DA', '2935', '2B72',
            '2EFA', '30A8', '310A', '3114', '311C', '32C5'],
}
# Units whose code is identical in all three episodes once relocations and DGROUP offsets are
# accounted for (unit_verdicts; M1-09): every E1 address in them maps to the same offset.
IDENTICAL_UNITS = {'Snd1E2A', 'WORX', 'Eng34E6', 'Eng386E', 'Crt', 'U3A92', 'System'}
UNIT_NAMES = ['Main', 'Game051A', 'Game0E8B', 'Snd1E2A', 'U1EBF', 'U1EDE', 'U2404', 'U2A15', 'WORX', 'U3292',
              'Eng34E6', 'Eng386E', 'Crt', 'Dos3A7E', 'U3A92', 'System', 'DGROUP']
EXE_FILE = {'file0001': 'file0001.exe', 'td2': 'td2.exe', 'td3': 'td3.exe'}

# Pairs fixed by hand (M1-09): the routine is called from the same call site of a matched
# caller, but its body changed too much for the similarity passes (episode-specific content).
FORCED = {
    'td2': {
        '0E8B:0984': '0E96:09A9',  # Sim_ProjectilesAndHits: 4th far call of the tick (Ghidra splits E2 at 0E96:1874)
        '0E8B:60D8': '0E96:60F9',  # Msn_Setup: called once per flight from Sim_FlyMission
        '2404:3521': '2236:240A',  # Msn_UpdateB: second mission-logic call of the tick
        '1EDE:0E6A': '1E0B:1475',  # per-flight message tables: only callee of Msn_Setup in unit 1EDE
    },
    'td3': {
        '2404:3521': '1FEB:19AD',  # Msn_UpdateB
        '1EDE:0E6A': '1BD8:12A1',  # per-flight message tables
    },
}

PREFIXES = {0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3}
MODRM1 = set()
for _r in range(0, 0x40, 8):
    MODRM1.update(range(_r, _r + 4))
MODRM1.update([0x62, 0x63, 0x69, 0x6B, 0xC0, 0xC1, 0xC4, 0xC5, 0xC6, 0xC7, 0xD0, 0xD1, 0xD2, 0xD3,
               0xF6, 0xF7, 0xFE, 0xFF])
MODRM1.update(range(0x80, 0x90))
MODRM1.update(range(0xD8, 0xE0))
NEAR_REL = set(range(0x70, 0x80)) | {0xE0, 0xE1, 0xE2, 0xE3, 0xE8, 0xE9, 0xEB}


def addr_key(a):
    s, o = a.split(':')
    return int(s, 16) * 16 + int(o, 16)


def fmt(seg, off):
    return '%04X:%04X' % (seg, off & 0xFFFF)


def layout(b):
    """Operand chunks of a 16-bit x86 instruction: list of (offset, width, kind)."""
    i = 0
    while i < len(b) and b[i] in PREFIXES:
        i += 1
    if i >= len(b):
        return [], None
    op = b[i]
    i += 1
    two = False
    if op == 0x0F and i < len(b):
        two = True
        op2 = b[i]
        i += 1
        has_modrm = not (0x80 <= op2 <= 0x8F or op2 in (0xA0, 0xA1, 0xA8, 0xA9, 0x31, 0x05, 0x06))
    else:
        has_modrm = op in MODRM1
    chunks = []
    if has_modrm and i < len(b):
        modrm = b[i]
        i += 1
        mod, rm = modrm >> 6, modrm & 7
        dl = 2 if (mod == 0 and rm == 6) or mod == 2 else 1 if mod == 1 else 0
        if dl:
            # BP-based addressing ([bp+..], [bp+si+..], [bp+di+..]) is a stack-frame offset
            stack = rm in (2, 3) or (rm == 6 and mod != 0)
            chunks.append((i, dl, 'sdisp' if stack else 'disp'))
            i += dl
    rest = len(b) - i
    kind = 'imm'
    if not two and op in NEAR_REL:
        kind = 'rel'
    if two and 0x80 <= op2 <= 0x8F:
        kind = 'rel'
    if not two and op in (0x9A, 0xEA) and rest == 4:
        chunks += [(i, 2, 'faroff'), (i + 2, 2, 'farseg')]
    elif rest == 3:  # enter imm16, imm8
        chunks += [(i, 2, kind), (i + 2, 1, kind)]
    elif rest == 4:
        chunks += [(i, 2, kind), (i + 2, 2, kind)]
    elif rest > 0:
        chunks.append((i, rest, kind))
    return chunks, (op if not two else 0x0F00 | op2)


class Insn:
    __slots__ = ('func', 'addr', 'lin', 'b', 'm', 'mn', 'refs', 'tok_m', 'tok_x')

    def __init__(self, func, addr, b, m, mn, refs):
        self.func = func
        self.addr = addr
        self.lin = addr_key(addr)
        self.b = bytes.fromhex(b)
        self.m = bytes.fromhex(m)
        self.mn = mn
        self.refs = [r for r in refs.split(';') if r] if refs else []
        self.tok_m = mn + ':' + bytes(x & y for x, y in zip(self.b, self.m)).hex()
        self.tok_x = self.tok_m if self.refs else mn + '=' + self.b.hex()


class Program:
    def __init__(self, exe, exports, data_dir):
        self.exe = exe
        self.insns = []
        with open(os.path.join(exports, exe + '-insns.tsv'), encoding='utf-8') as f:
            next(f)
            for line in f:
                p = line.rstrip('\n').split('\t')
                while len(p) < 6:
                    p.append('')
                self.insns.append(Insn(*p[:6]))
        self.funcs = {}
        with open(os.path.join(exports, exe + '-funcs.tsv'), encoding='utf-8') as f:
            next(f)
            for line in f:
                p = line.rstrip('\n').split('\t')
                self.funcs[p[0]] = {'name': p[1], 'bytes': int(p[2]), 'n': int(p[3])}
        self.body = collections.defaultdict(list)
        self.by_addr = {}
        for i in self.insns:
            self.by_addr[i.addr] = i
            if i.func != '-':
                self.body[i.func].append(i)
        for k in self.body:
            self.body[k].sort(key=lambda x: x.lin)
        self.segs = [int(x, 16) for x in SEGS[exe]]
        self.dgroup = self.segs[-1]
        self.img, self.relocs = load_image(os.path.join(data_dir, EXE_FILE[exe]))
        self.unit_size = [(self.segs[i + 1] - self.segs[i]) * 16 if i + 1 < len(self.segs)
                          else len(self.img) - self.segs[i] * 16 for i in range(len(self.segs))]
        self.unit = {e: self.unit_of(e) for e in self.body}
        self.feat = {}
        for e, body in self.body.items():
            self.feat[e] = {
                'x': hashlib.sha1('|'.join(i.tok_x for i in body).encode()).hexdigest(),
                'm': hashlib.sha1('|'.join(i.tok_m for i in body).encode()).hexdigest(),
                'toks': [i.tok_m for i in body],
                'calls': calls_of(body),
            }

    def unit_of(self, a):
        s = int(a[:4], 16)
        return max(i for i, b in enumerate(self.segs) if s >= b)

    def pstring(self, seg, off):
        p = seg * 16 + off
        if p >= len(self.img):
            return None
        n = self.img[p]
        if n == 0 or p + 1 + n > len(self.img):
            return None
        s = self.img[p + 1:p + 1 + n]
        if sum(32 <= c < 127 for c in s) < n * 0.9:
            return None
        return s


def load_image(path):
    b = open(path, 'rb').read()
    cblp, cp, crlc, cparhdr = struct.unpack_from('<HHHH', b, 2)
    lfarlc = struct.unpack_from('<H', b, 0x18)[0]
    size = cp * 512 - (512 - cblp if cblp else 0)
    img = b[cparhdr * 16:size]
    rel = set()
    for i in range(crlc):
        off, seg = struct.unpack_from('<HH', b, lfarlc + 4 * i)
        rel.add(seg * 16 + off)
    return img, rel


def calls_of(body):
    out = []
    for i in body:
        if i.mn in ('CALL', 'CALLF'):
            for r in i.refs:
                if r[0] == 'F':
                    out.append(r[1:])
    return out


def sim(a, b):
    if not a or not b:
        return 0.0
    sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
    if sm.real_quick_ratio() < 0.3 or sm.quick_ratio() < 0.3:
        return sm.quick_ratio() * 0.5
    return sm.ratio()


class Matcher:
    def __init__(self, p1, p2):
        self.p1, self.p2 = p1, p2
        self.m12 = {}
        self.m21 = {}
        self.how = {}
        self.dsmap = {}
        self.ds_sorted = []
        self.ds_delta = {}
        self.refvotes = collections.defaultdict(collections.Counter)

    def add(self, a, b, how):
        if a in self.m12 or b in self.m21:
            return False
        self.m12[a] = b
        self.m21[b] = a
        self.how[a] = how
        return True

    # ------------------------------------------------------------------ instruction comparison
    def walk(self, a, b_entry, learn=None):
        """Walk E1 function a over the target image at b_entry.
        Returns (skeleton_equal, n_insns, unexplained_diffs)."""
        p1, p2 = self.p1, self.p2
        body = p1.body[a]
        e1 = addr_key(a)
        e2 = addr_key(b_entry)
        seg2 = int(b_entry[:4], 16)
        diffs = []
        ctx = (a, b_entry, None)
        for x in body:
            lin2 = e2 + (x.lin - e1)
            n = len(x.b)
            yb = p2.img[lin2:lin2 + n]
            if len(yb) < n:
                return False, len(body), diffs
            if any((u & m) != (v & m) for u, v, m in zip(x.b, yb, x.m)):
                return False, len(body), diffs
            if learn is not None:
                y = p2.by_addr.get(fmt(seg2, lin2 - seg2 * 16))
                if y is not None and len(y.b) == n:
                    self.vote_refs(x, y)
            if yb != x.b:
                diffs += self.cmp_insn(x, yb, lin2, seg2, ctx, learn)
        return True, len(body), diffs

    def vote_refs(self, x, y):
        dx = [r[1:] for r in x.refs if r[0] == 'D']
        dy = [r[1:] for r in y.refs if r[0] == 'D']
        if len(dx) == len(dy):
            for u, v in zip(dx, dy):
                self.refvotes[u][v] += 1

    def align(self, a, b, learn=None):
        """Align a changed pair on the skeleton tokens. Returns (hunks, diffs): hunks are the
        non-equal runs (tag, e1 first, e1 last, n1, e2 first, e2 last, n2); diffs are operand
        differences inside the equal runs that no address explains."""
        A, B = self.p1.body[a], self.p2.body.get(b, [])
        ta = [i.tok_m for i in A]
        tb = [i.tok_m for i in B]
        sm = difflib.SequenceMatcher(None, ta, tb, autojunk=False)
        ops = sm.get_opcodes()
        local = {}
        for tag, i1, i2, j1, j2 in ops:
            if tag == 'equal':
                for x, y in zip(A[i1:i2], B[j1:j2]):
                    local[x.addr] = y.addr
        hunks, diffs = [], []
        ctx = (a, b, local)
        for tag, i1, i2, j1, j2 in ops:
            if tag == 'equal':
                for x, y in zip(A[i1:i2], B[j1:j2]):
                    if learn is not None:
                        self.vote_refs(x, y)
                    if x.b != y.b:
                        diffs += self.cmp_insn(x, y.b, y.lin, int(y.addr[:4], 16), ctx, learn)
            else:
                e1f = A[i1].addr if i1 < len(A) else ''
                e2f = B[j1].addr if j1 < len(B) else ''
                hunks.append((tag, e1f, A[i2 - 1].addr if i1 < i2 else '', i2 - i1,
                              e2f, B[j2 - 1].addr if j1 < j2 else '', j2 - j1))
        return hunks, diffs

    def cmp_insn(self, x, yb, lin2, seg2, ctx, learn):
        out = []
        chunks, op = layout(x.b)
        covered = set()
        for off, w, kind in chunks:
            covered.update(range(off, off + w))
            v1 = int.from_bytes(x.b[off:off + w], 'little')
            v2 = int.from_bytes(yb[off:off + w], 'little')
            if v1 == v2:
                continue
            if self.explain(x, yb, lin2, seg2, ctx, off, w, kind, v1, v2, learn) is None:
                out.append((x.addr, fmt(seg2, lin2 - seg2 * 16), x.mn, kind, w, v1, v2))
        for k in range(len(x.b)):
            if k not in covered and x.b[k] != yb[k]:
                out.append((x.addr, fmt(seg2, lin2 - seg2 * 16), x.mn, 'opcode', 1, x.b[k], yb[k]))
        return out

    def explain(self, x, yb, lin2, seg2, ctx, off, w, kind, v1, v2, learn):
        p1, p2 = self.p1, self.p2
        if w == 2 and (x.lin + off) in p1.relocs and (lin2 + off) in p2.relocs:
            return 'reloc'
        seg1 = int(x.addr[:4], 16)
        ip1 = x.lin - seg1 * 16
        ip2 = lin2 - seg2 * 16
        if kind == 'rel':
            t1 = fmt(seg1, ip1 + len(x.b) + sign(v1, w))
            t2 = fmt(seg2, ip2 + len(x.b) + sign(v2, w))
            return 'flow' if self.code_corr(t1, t2, ctx) else None
        if kind == 'faroff':
            s1 = int.from_bytes(x.b[off + 2:off + 4], 'little')
            s2 = int.from_bytes(yb[off + 2:off + 4], 'little')
            return 'flow' if self.code_corr(fmt(s1, v1), fmt(s2, v2), ctx) else None
        if w != 2 or kind == 'sdisp':
            return None
        # code pointer to a matched function in the own segment (push cs; push offset proc)
        if self.code_corr(fmt(seg1, v1), fmt(seg2, v2), ctx, strict=True):
            return 'codeptr'
        # Pascal string literal with the same text in the own segment
        s1 = p1.pstring(seg1, v1)
        if s1 is not None and s1 == p2.pstring(seg2, v2):
            return 'string'
        if learn is not None:
            learn[fmt(p1.dgroup, v1)][fmt(p2.dgroup, v2)] += 1
        if self.ds_ok(v1, v2):
            return 'ds'
        return None

    def code_corr(self, t1, t2, ctx, strict=False):
        if self.m12.get(t1) == t2:
            return True
        if strict:
            return False
        a, b_entry, local = ctx
        if local is not None:
            if local.get(t1) == t2:
                return True
        elif t1[:4] == a[:4] and t2[:4] == b_entry[:4]:
            # same position relative to the matched entries (jumps inside the function)
            if int(t1[5:], 16) - int(a[5:], 16) == int(t2[5:], 16) - int(b_entry[5:], 16):
                return True
        # same offset in a unit of the same size that sits at the corresponding position
        u1 = self.p1.unit_of(t1)
        u2 = self.p2.unit_of(t2)
        if u1 == u2 and self.p1.unit_size[u1] == self.p2.unit_size[u2] and t1[5:] == t2[5:]:
            return True
        return False

    def ds_ok(self, v1, v2):
        k = fmt(self.p1.dgroup, v1)
        if k in self.dsmap:
            return self.dsmap[k] == fmt(self.p2.dgroup, v2)
        # neighbour delta: same delta as the nearest accepted offsets below and above
        i = bisect.bisect_left(self.ds_sorted, v1)
        d = v2 - v1
        lo = self.ds_sorted[i - 1] if i > 0 else None
        hi = self.ds_sorted[i] if i < len(self.ds_sorted) else None
        dl = self.ds_delta[lo] if lo is not None else None
        dh = self.ds_delta[hi] if hi is not None else None
        return d == dl and d == dh or (d == dl and hi is None) or (d == dh and lo is None)

    def learn_dsmap(self):
        learn = collections.defaultdict(collections.Counter)
        for a, b in self.m12.items():
            ok, _, _ = self.walk(a, b, learn)
            if not ok:
                self.align(a, b, learn)
        m = {}
        for k1, c in learn.items():
            k2, n = c.most_common(1)[0]
            tot = sum(c.values())
            if n >= 2 and n * 3 >= tot * 2:
                m[k1] = k2
        self.dsmap = m
        self.ds_sorted = sorted(int(k[5:], 16) for k in m)
        self.ds_delta = {int(k[5:], 16): int(v[5:], 16) - int(k[5:], 16) for k, v in m.items()}
        # accept single votes whose delta matches both neighbours
        for k1, c in learn.items():
            if k1 in m:
                continue
            k2, n = c.most_common(1)[0]
            if n == sum(c.values()) and self.ds_ok(int(k1[5:], 16), int(k2[5:], 16)):
                m[k1] = k2
        # accept runs of at least three unanimous offsets that share one delta and have no
        # accepted offset with another delta between them
        single = sorted((int(k1[5:], 16), int(c.most_common(1)[0][0][5:], 16)) for k1, c in learn.items()
                        if k1 not in m and len(c) == 1)
        acc = sorted(self.ds_delta.items())
        run = []
        for v1, v2 in single + [(None, None)]:
            if run and (v1 is None or v2 - v1 != run[0][1] - run[0][0] or
                        any(run[-1][0] < k < v1 and d != run[0][1] - run[0][0] for k, d in acc)):
                if len(run) >= 3:
                    for r1, r2 in run:
                        m[fmt(self.p1.dgroup, r1)] = fmt(self.p2.dgroup, r2)
                run = []
            if v1 is not None:
                run.append((v1, v2))
        self.dsmap = m
        self.ds_sorted = sorted(int(k[5:], 16) for k in m)
        self.ds_delta = {int(k[5:], 16): int(v[5:], 16) - int(k[5:], 16) for k, v in m.items()}
        # once more: single votes whose delta now matches their neighbours
        for k1, c in learn.items():
            k2, n = c.most_common(1)[0]
            if k1 not in m and n == sum(c.values()) and self.ds_ok(int(k1[5:], 16), int(k2[5:], 16)):
                m[k1] = k2
        self.ds_sorted = sorted(int(k[5:], 16) for k in m)
        self.ds_delta = {int(k[5:], 16): int(v[5:], 16) - int(k[5:], 16) for k, v in m.items()}
        return learn

    def unit_verdicts(self):
        """Same-size units: walk every E1 instruction of the unit (also those outside Ghidra
        functions) at the same offset in the target, and compare the remaining bytes raw."""
        out = []
        p1, p2 = self.p1, self.p2
        by_seg = collections.defaultdict(list)
        for x in p1.insns:
            by_seg[p1.unit_of(x.addr)].append(x)
        for u in range(len(UNIT_NAMES) - 1):
            if p1.unit_size[u] != p2.unit_size[u]:
                out.append((u, 'size differs', p1.unit_size[u], p2.unit_size[u], '', '', ''))
                continue
            b1 = p1.segs[u] * 16
            b2 = p2.segs[u] * 16
            covered = set()
            skel = 0
            diffs = []
            ctx = (fmt(p1.segs[u], 0), fmt(p2.segs[u], 0), None)
            for x in by_seg[u]:
                lin2 = b2 + (x.lin - b1)
                yb = p2.img[lin2:lin2 + len(x.b)]
                covered.update(range(x.lin, x.lin + len(x.b)))
                if any((s & m) != (t & m) for s, t, m in zip(x.b, yb, x.m)):
                    skel += 1
                    continue
                if yb != x.b:
                    diffs += self.cmp_insn(x, yb, lin2, p2.segs[u], ctx, None)
            raw = 0
            for k in range(p1.unit_size[u]):
                if b1 + k in covered:
                    continue
                if p1.img[b1 + k] != p2.img[b2 + k]:
                    r1 = (b1 + k) in p1.relocs or (b1 + k - 1) in p1.relocs
                    if not r1:
                        raw += 1
            out.append((u, 'same size', p1.unit_size[u], p2.unit_size[u], skel, len(diffs), raw))
        return out

    # ------------------------------------------------------------------ passes
    def unmatched(self, prog, side, unit=None):
        m = self.m12 if side == 1 else self.m21
        return sorted([e for e in prog.feat if e not in m and (unit is None or prog.unit[e] == unit)],
                      key=addr_key)

    def offset_pass(self):
        n = 0
        for u in range(len(UNIT_NAMES) - 1):
            if self.p1.unit_size[u] != self.p2.unit_size[u]:
                continue
            s1 = SEGS[self.p1.exe][u]
            s2 = SEGS[self.p2.exe][u]
            for a in self.unmatched(self.p1, 1, u):
                b = s2 + a[4:]
                if a[:4] != s1:
                    continue
                ok, _, _ = self.walk(a, b)
                if ok:
                    n += self.add(a, b, 'offset')
        return n

    def unique_key_pass(self, key, how):
        n = 0
        for u in range(len(UNIT_NAMES)):
            a = collections.defaultdict(list)
            b = collections.defaultdict(list)
            for e in self.unmatched(self.p1, 1, u):
                a[self.p1.feat[e][key]].append(e)
            for e in self.unmatched(self.p2, 2, u):
                b[self.p2.feat[e][key]].append(e)
            for k, l1 in a.items():
                l2 = b.get(k, [])
                if len(l1) == 1 and len(l2) == 1:
                    n += self.add(l1[0], l2[0], how)
                elif len(l1) == len(l2) and len(l1) > 1:
                    for x, y in zip(l1, l2):
                        n += self.add(x, y, how + '-dup')
        return n

    def search_pass(self):
        n = 0
        for a in self.unmatched(self.p1, 1):
            body = self.p1.body[a]
            if len(body) < 6:
                continue
            u = self.p1.unit[a]
            s2 = self.p2.segs[u]
            base = s2 * 16
            size = self.p2.unit_size[u]
            first = body[0]
            hits = []
            img = self.p2.img
            # candidate starts: first instruction's skeleton matches
            for off in range(0, size - len(first.b)):
                lin = base + off
                if all((img[lin + k] & first.m[k]) == (first.b[k] & first.m[k]) for k in range(len(first.b))):
                    b = fmt(s2, off)
                    if b in self.m21:
                        continue
                    ok, _, _ = self.walk(a, b)
                    if ok:
                        hits.append(b)
                        if len(hits) > 1:
                            break
            if len(hits) == 1:
                n += self.add(a, hits[0], 'search')
        return n

    def callgraph_pass(self, thresh=0.5, single_only=False, how='callgraph'):
        """Align the call-site lists of matched pairs; pair unmatched callees that sit in the
        same place. With single_only, only 1:1 replacements are taken (any similarity)."""
        n = 0
        for a, b in list(self.m12.items()):
            if b not in self.p2.feat:
                continue
            c1 = self.p1.feat[a]['calls']
            c2 = self.p2.feat[b]['calls']
            t1 = [self.m12.get(x, '?' + x) for x in c1]
            sm = difflib.SequenceMatcher(None, t1, c2, autojunk=False)
            for tag, i1, i2, j1, j2 in sm.get_opcodes():
                if tag != "replace" or i2 - i1 != j2 - j1 or (single_only and i2 - i1 > 3):
                    continue
                for x, y in zip(c1[i1:i2], c2[j1:j2]):
                    if x in self.p1.feat and y in self.p2.feat and x not in self.m12 and y not in self.m21 \
                            and self.p1.unit[x] == self.p2.unit[y]:
                        if sim(self.p1.feat[x]['toks'], self.p2.feat[y]['toks']) >= thresh:
                            n += self.add(x, y, how)
        return n

    def order_pass(self, thresh):
        n = 0
        for u in range(len(UNIT_NAMES)):
            f1 = sorted([e for e in self.p1.feat if self.p1.unit[e] == u], key=addr_key)
            f2 = sorted([e for e in self.p2.feat if self.p2.unit[e] == u], key=addr_key)
            idx2 = {e: i for i, e in enumerate(f2)}
            anchors = [(-1, -1)]
            for i, e in enumerate(f1):
                if e in self.m12 and self.m12[e] in idx2:
                    anchors.append((i, idx2[self.m12[e]]))
            anchors.append((len(f1), len(f2)))
            for (a0, b0), (a1, b1) in zip(anchors, anchors[1:]):
                g1 = [e for e in f1[a0 + 1:a1] if e not in self.m12]
                g2 = [e for e in f2[b0 + 1:b1] if e not in self.m21] if b1 > b0 else []
                if not g1 or not g2:
                    continue
                if len(g1) == len(g2):
                    for x, y in zip(g1, g2):
                        if sim(self.p1.feat[x]['toks'], self.p2.feat[y]['toks']) >= thresh:
                            n += self.add(x, y, 'order')
                else:
                    cands = sorted(((sim(self.p1.feat[x]['toks'], self.p2.feat[y]['toks']), x, y)
                                    for x in g1 for y in g2), reverse=True)
                    for s, x, y in cands:
                        if s >= max(thresh, 0.6):
                            n += self.add(x, y, 'order')
        return n

    def best_pass(self, thresh):
        n = 0
        for u in range(len(UNIT_NAMES)):
            g1 = self.unmatched(self.p1, 1, u)
            g2 = self.unmatched(self.p2, 2, u)
            cands = []
            for x in g1:
                for y in g2:
                    s = sim(self.p1.feat[x]['toks'], self.p2.feat[y]['toks'])
                    if s >= thresh:
                        cands.append((s, x, y))
            for s, x, y in sorted(cands, reverse=True):
                n += self.add(x, y, 'best')
        return n

    def run(self):
        forced = 0
        for a, b in FORCED.get(self.p2.exe, {}).items():
            forced += self.add(a, b, 'manual')
        log = [('manual', forced),
               ('offset', self.offset_pass()),
               ('exact', self.unique_key_pass('x', 'exact')),
               ('shape', self.unique_key_pass('m', 'shape')),
               ('search', self.search_pass())]
        for _ in range(6):
            k = 0
            for _ in range(6):
                j = self.callgraph_pass() + self.callgraph_pass(0.15, True, 'callsite')
                k += j
                if not j:
                    break
            k += self.order_pass(0.5)
            log.append(('callgraph+callsite+order', k))
            if not k:
                break
        log.append(('best', self.best_pass(0.70)))
        log.append(('order-late', self.order_pass(0.35)))
        return log

    def align_map(self, a, b):
        A, B = self.p1.body[a], self.p2.body.get(b, [])
        sm = difflib.SequenceMatcher(None, [i.tok_m for i in A], [i.tok_m for i in B], autojunk=False)
        m = {}
        for tag, i1, i2, j1, j2 in sm.get_opcodes():
            if tag == 'equal':
                for x, y in zip(A[i1:i2], B[j1:j2]):
                    m[x.addr] = y.addr
        return m

    def classify(self):
        res = {}
        for a, b in self.m12.items():
            ok, n, diffs = self.walk(a, b)
            if ok:
                res[a] = ('IDENT' if not diffs else 'CONST', 1.0, diffs)
            else:
                tb = self.p2.feat[b]['toks'] if b in self.p2.feat else []
                hunks, diffs = self.align(a, b)
                res[a] = ('CHANGED', sim(self.p1.feat[a]['toks'], tb), diffs, hunks)
        return res


CONF_ORDER = ['HYPOTHESIS', 'LIKELY', 'CONFIRMED']


def cap(conf, limit):
    if conf not in CONF_ORDER:
        conf = 'HYPOTHESIS'
    return CONF_ORDER[min(CONF_ORDER.index(conf), CONF_ORDER.index(limit))]


def short(text, n=110):
    text = ' '.join(text.split())
    return text if len(text) <= n else text[:n - 3].rstrip(' ,;') + '...'


def transfer_symbols(mt, res, rows, task):
    """E1 symbol rows -> rows for the target exe (same CSV format). Returns (rows, stats)."""
    p1, p2 = mt.p1, mt.p2
    t = p2.exe
    out = []
    stats = collections.Counter()
    # data-reference votes from aligned instruction pairs, as offset deltas per E1 offset
    ref = {}
    for u, c in mt.refvotes.items():
        v, n = c.most_common(1)[0]
        ref[u] = (v, n, sum(c.values()))
    dg1 = '%04X' % p1.dgroup
    ref_dg = sorted((int(u[5:], 16), int(v[0][5:], 16) - int(u[5:], 16), v[1], v[2]) for u, v in ref.items()
                    if u[:4] == dg1 and v[0][:4] == '%04X' % p2.dgroup)
    keys = [r[0] for r in ref_dg]

    def ds_lookup(off):
        k = fmt(p1.dgroup, off)
        if k in ref and ref[k][0][:4] == '%04X' % p2.dgroup:
            v, n, tot = ref[k]
            return v, ('refs %d/%d' % (n, tot)), 'CONFIRMED' if n >= 2 and n == tot else 'LIKELY'
        if k in mt.dsmap:
            return mt.dsmap[k], 'operand map', 'LIKELY'
        # inside a structure or array: the delta of the nearest referenced offsets on both sides
        i = bisect.bisect_right(keys, off)
        lo = ref_dg[i - 1] if i > 0 else None
        hi = ref_dg[i] if i < len(ref_dg) else None
        if lo and hi and lo[1] == hi[1] and lo[2] >= 2 and hi[2] >= 2:
            return fmt(p2.dgroup, off + lo[1]), 'between %04X and %04X' % (lo[0], hi[0]), 'HYPOTHESIS'
        return None, None, None

    for r in rows:
        a = r['address']
        seg = int(a[:4], 16)
        u = p1.unit_of(a)
        new = dict(r)
        new['source_task'] = task
        e1 = 'file0001:' + a
        target = None
        if r['kind'] == 'func' and a in mt.m12:
            target = mt.m12[a]
            c = res[a][0]
            if c == 'IDENT':
                conf, note = r['confidence'], 'identical'
            elif c == 'CONST':
                conf, note = cap(r['confidence'], 'LIKELY'), 'same code, %d operand(s) differ' % len(res[a][2])
            else:
                sm = res[a][1]
                conf = cap(r['confidence'], 'LIKELY' if sm >= 0.6 or mt.how[a] == 'manual' else 'HYPOTHESIS')
                note = 'changed (similarity %.2f)' % sm
            stats['func-' + c] += 1
        elif u < len(p1.segs) - 1 and p1.unit_size[u] == p2.unit_size[u] and \
                UNIT_NAMES[u] in IDENTICAL_UNITS:
            target = fmt(p2.segs[u], int(a[5:], 16))
            conf, note = r['confidence'], 'identical unit, same offset'
            stats['unit-offset'] += 1
        elif r['kind'] in ('label', 'func') and a in p1.by_addr and p1.by_addr[a].func in mt.m12:
            f = p1.by_addr[a].func
            fb = mt.m12[f]
            c = res[f][0]
            if c in ('IDENT', 'CONST'):
                target = fmt(int(fb[:4], 16), int(fb[5:], 16) + addr_key(a) - addr_key(f))
                conf, note = cap(r['confidence'], 'LIKELY' if c == 'CONST' else 'CONFIRMED'), \
                    'same place in %s (%s)' % (f, c.lower())
            else:
                loc = mt.align_map(f, fb).get(a)
                if loc:
                    target = loc
                    conf, note = cap(r['confidence'], 'LIKELY'), 'aligned instruction in changed %s' % f
            if target:
                stats['label'] += 1
        elif r['kind'] == 'data' and seg == p1.dgroup:
            target, how, lim = ds_lookup(int(a[5:], 16))
            if target:
                conf, note = cap(r['confidence'], lim), 'DS offset by ' + how
                stats['data-' + lim] += 1
        if target is None:
            stats['dropped-' + r['kind']] += 1
            continue
        new['address'] = target
        new['confidence'] = conf
        new['comment'] = short('= E1 %s, %s. E1: %s' % (e1, note, r['comment']), 200)
        out.append(new)
    out.sort(key=lambda x: addr_key(x['address']))
    # one row per address (two E1 rows can land on one address only through a bad match)
    seen = {}
    for x in out:
        if x['address'] in seen:
            stats['duplicate-address'] += 1
        seen.setdefault(x['address'], x)
    return list(seen.values()), stats


def sign(v, w):
    lim = 1 << (8 * w)
    return v - lim if v >= lim // 2 else v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--exports', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--data', default=os.environ.get('TD2192_DATA', 'P:/msdos/TD2192/TD2192'))
    ap.add_argument('--targets', default='td2,td3')
    ap.add_argument('--symbols', help='file0001.csv: also write <out>/<target>.csv with transferred names')
    ap.add_argument('--task', default='M1-09')
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    p1 = Program('file0001', args.exports, args.data)
    for t in args.targets.split(','):
        p2 = Program(t, args.exports, args.data)
        mt = Matcher(p1, p2)
        log = mt.run()
        learn = mt.learn_dsmap()
        res = mt.classify()
        with open(os.path.join(args.out, 'match-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1', t, 'unit', 'how', 'class', 'sim', 'n1', 'n2', 'ndiff', 'e1name'])
            for a in sorted(p1.feat, key=addr_key):
                nm = p1.funcs.get(a, {}).get('name', '')
                if a in mt.m12:
                    b = mt.m12[a]
                    c, s, d = res[a][:3]
                    w.writerow([a, b, UNIT_NAMES[p1.unit[a]], mt.how[a], c, '%.3f' % s,
                                len(p1.body[a]), len(p2.body.get(b, [])), len(d), nm])
                else:
                    w.writerow([a, '', UNIT_NAMES[p1.unit[a]], '', 'GONE', '', len(p1.body[a]), '', '', nm])
            for b in sorted(p2.feat, key=addr_key):
                if b not in mt.m21:
                    w.writerow(['', b, UNIT_NAMES[p2.unit[b]], '', 'NEW', '', '', len(p2.body[b]), '', ''])
        with open(os.path.join(args.out, 'constdiff-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1func', 'func', 'class', 'e1addr', 'addr', 'mnemonic', 'kind', 'width', 'e1value',
                        'value'])
            for a in sorted(res, key=addr_key):
                for d in res[a][2]:
                    w.writerow([a, mt.m12[a], res[a][0], d[0], d[1], d[2], d[3], d[4], '%X' % d[5], '%X' % d[6]])
        with open(os.path.join(args.out, 'hunks-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1func', 'func', 'tag', 'e1first', 'e1last', 'n1', 'first', 'last', 'n2'])
            for a in sorted(res, key=addr_key):
                if res[a][0] == 'CHANGED':
                    for h in res[a][3]:
                        w.writerow([a, mt.m12[a]] + list(h))
        with open(os.path.join(args.out, 'dsmap-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1', t, 'votes', 'total', 'alternatives', 'accepted'])
            for u in sorted(learn, key=addr_key):
                c = learn[u]
                v, n = c.most_common(1)[0]
                w.writerow([u, v, n, sum(c.values()), len(c), int(mt.dsmap.get(u) == v)])
        with open(os.path.join(args.out, 'refmap-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1', t, 'votes', 'total', 'alternatives'])
            for u in sorted(mt.refvotes, key=addr_key):
                c = mt.refvotes[u]
                v, n = c.most_common(1)[0]
                w.writerow([u, v, n, sum(c.values()), len(c)])
        with open(os.path.join(args.out, 'pairs-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['e1', t, 'how', 'class'])
            for a in sorted(mt.m12, key=addr_key):
                w.writerow([a, mt.m12[a], mt.how[a], res[a][0]])
        with open(os.path.join(args.out, 'units-%s.tsv' % t), 'w', encoding='utf-8', newline='') as f:
            w = csv.writer(f, delimiter='\t')
            w.writerow(['unit', 'e1seg', 'seg', 'verdict', 'e1size', 'size', 'skeleton_mismatch',
                        'operand_diffs', 'raw_byte_diffs'])
            for u, v, s1, s2, sk, od, rb in mt.unit_verdicts():
                w.writerow([UNIT_NAMES[u], SEGS['file0001'][u], SEGS[t][u], v, '%X' % s1, '%X' % s2, sk, od, rb])
        if args.symbols:
            rows = list(csv.DictReader(open(args.symbols, encoding='utf-8')))
            fields = ['address', 'kind', 'name', 'signature', 'comment', 'confidence', 'source_task']
            out, st = transfer_symbols(mt, res, rows, args.task)
            with open(os.path.join(args.out, '%s.csv' % t), 'w', encoding='utf-8', newline='') as f:
                w = csv.DictWriter(f, fieldnames=fields, lineterminator='\n')
                w.writeheader()
                for r in out:
                    w.writerow({k: r.get(k, '') for k in fields})
            print(t, 'symbols', len(out), 'of', len(rows), dict(st))
        cnt = collections.Counter(r[0] for r in res.values())
        print(t, 'passes', log)
        print(t, 'E1 functions', len(p1.feat), t, 'functions', len(p2.feat), 'matched', len(mt.m12),
              dict(cnt), 'gone', len(p1.feat) - len(mt.m12), 'new', len(p2.feat) - len(set(mt.m21) & set(p2.feat)),
              'dsmap', len(mt.dsmap))
        per_unit = collections.defaultdict(collections.Counter)
        for a in p1.feat:
            per_unit[UNIT_NAMES[p1.unit[a]]][res[a][0] if a in res else 'GONE'] += 1
        for b in p2.feat:
            if b not in mt.m21:
                per_unit[UNIT_NAMES[p2.unit[b]]]['NEW'] += 1
        for u in UNIT_NAMES:
            if per_unit[u]:
                print('  %-10s %s' % (u, dict(per_unit[u])))


if __name__ == '__main__':
    sys.exit(main())
