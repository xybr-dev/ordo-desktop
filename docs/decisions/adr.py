#!/usr/bin/env python3
"""Decision records in docs/decisions/: start one, rebuild the index, check them all.

    python docs/decisions/adr.py new "Receipts expire after a year" --tags storage,privacy [--date 2026-10-14]
    python docs/decisions/adr.py index     # rewrite the table in docs/decisions/README.md
    python docs/decisions/adr.py check     # exit 1 on any problem; CI and `cargo xtask verify` run this

A copy of clockout's docs/decisions/adr.py (itself a copy of personal-admin-os's), so every series of
records reads the same. It lives beside the records and needs no paths.py. Ordo adds one thing:
`amends` / `amended_by` for a decision that replaces part of an earlier one, which stays accepted.

One decision per file, NNNN-kebab-title.md, with the frontmatter in TEMPLATE.md. Numbers follow the
order records are written and are never reused; `date` is when the decision was made, `recorded` when
it was written down (they differ for backfilled ones). An accepted record is not rewritten: a changed
decision gets a new record with `supersedes: [N]`, and N becomes `superseded` with `superseded_by`;
a partly changed one gets `amends: [N]`, and N gains `amended_by` and a forward notice.

The frontmatter is a flat subset of YAML (key: value, inline [lists], null), parsed here so the
tool needs nothing beyond the standard library.
"""
import argparse
import datetime as dt
import re
import sys
import pathlib
from types import SimpleNamespace

_here = pathlib.Path(__file__).resolve().parent
paths = SimpleNamespace(
    ROOT=_here.parent.parent,
    DECISIONS=_here,
    DECISIONS_INDEX=_here / 'README.md',
    DECISIONS_TEMPLATE=_here / 'TEMPLATE.md',
)

FIELDS   = ('id', 'title', 'status', 'date', 'recorded', 'tags', 'supersedes', 'superseded_by',
            'amends', 'amended_by', 'sources')
REQUIRED = ('id', 'title', 'status', 'date', 'recorded')
STATUSES = ('proposed', 'accepted', 'superseded', 'deprecated', 'rejected')
NAME     = re.compile(r'^(\d{4})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$')
KEBAB    = re.compile(r'^[a-z0-9]+(?:-[a-z0-9]+)*$')
START, END = '<!-- adr-index:start -->', '<!-- adr-index:end -->'


# ── reading ─────────────────────────────────────────────────────────────────
def _value(raw):
    raw = raw.strip()
    if raw[:1] not in ('"', "'"):
        raw = re.sub(r'\s+#.*$', '', raw)           # trailing comment, unless the value is quoted
    if raw in ('', 'null', '~'):
        return None
    if raw.startswith('[') and raw.endswith(']'):
        return [_value(x) for x in raw[1:-1].split(',') if x.strip()]
    if re.fullmatch(r'-?\d+', raw):
        return int(raw)
    if len(raw) >= 2 and raw[0] == raw[-1] == '"':
        return re.sub(r'\\(["\\])', r'\1', raw[1:-1])
    if len(raw) >= 2 and raw[0] == raw[-1] == "'":
        return raw[1:-1].replace("''", "'")
    return raw


def _scalar(s):
    """A string as a frontmatter value, quoted when YAML would otherwise misread it."""
    if re.search(r'[:#"\'\[\]{}|>&*!%@,`]', s) or s != s.strip():
        return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'
    return s


def parse(path):
    """-> (frontmatter dict, body). Raises ValueError on malformed frontmatter."""
    text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
    m = re.match(r'---\n(.*?)\n---\n(.*)', text, re.S)
    if not m:
        raise ValueError('no frontmatter: the file must start with --- ... ---')
    meta = {}
    for n, line in enumerate(m.group(1).splitlines(), 2):
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        key, sep, val = line.partition(':')
        if not sep or key != key.strip() or not key:
            raise ValueError(f'line {n}: expected "key: value", got {line!r}')
        if key in meta:
            raise ValueError(f'line {n}: {key} given twice')
        meta[key] = _value(val)
    return meta, m.group(2)


def records():
    """Every NNNN-*.md in docs/decisions, sorted by number -> [(path, meta | None, body, error)]."""
    out = []
    for p in sorted(paths.DECISIONS.glob('[0-9][0-9][0-9][0-9]-*.md')):
        try:
            meta, body = parse(p)
            out.append((p, meta, body, None))
        except ValueError as e:
            out.append((p, None, '', str(e)))
    return out


# ── the index ───────────────────────────────────────────────────────────────
def _link(n, by_id):
    p = by_id.get(n)
    return f'[{n:04d}]({p.name})' if p else f'{n:04d}'


def render(recs):
    ok = [(p, m) for p, m, _, err in recs if not err]
    by_id = {m.get('id'): p for p, m in ok}
    rows = ['| # | Decision | Status | Decided | Tags |', '|---|---|---|---|---|']
    for p, m in ok:
        status = m.get('status') or ''
        if status == 'superseded' and isinstance(m.get('superseded_by'), int):
            status = f'superseded by {_link(m["superseded_by"], by_id)}'
        amended = [a for a in m.get('amended_by') or [] if isinstance(a, int)]
        if amended:
            status += ', amended by ' + ', '.join(_link(a, by_id) for a in amended)
        title = str(m.get('title') or '').replace('|', '\\|')
        tags = ', '.join(str(t) for t in (m.get('tags') or []))
        rows.append(f'| [{p.name[:4]}]({p.name}) | {title} | {status} | {m.get("date") or ""} | {tags} |')
    note = f'<!-- generated by `python docs/decisions/adr.py index`; edit the records, not this table -->'
    return '\n'.join([START, note, '', *rows, '', END])


def _with_index(readme_text, recs):
    i, j = readme_text.find(START), readme_text.find(END)
    if i < 0 or j < i:
        raise ValueError(f'{paths.DECISIONS_INDEX.name} has no {START} ... {END} block')
    return readme_text[:i] + render(recs) + readme_text[j + len(END):]


def write_index(recs=None):
    recs = records() if recs is None else recs
    text = paths.DECISIONS_INDEX.read_text(encoding='utf-8')
    new = _with_index(text, recs)
    if new != text:
        paths.DECISIONS_INDEX.write_text(new, encoding='utf-8', newline='\n')
    return new != text


# ── checking ────────────────────────────────────────────────────────────────
def _date(v):
    try:
        return dt.date.fromisoformat(str(v))
    except ValueError:
        return None


def problems(recs):
    errs = []
    by_id = {}
    for p, m, body, err in recs:
        name = p.name
        if err:
            errs.append(f'{name}: {err}')
            continue
        if not NAME.match(name):
            errs.append(f'{name}: file name must be NNNN-kebab-title.md (lowercase, digits, hyphens)')
        for k in REQUIRED:
            if m.get(k) in (None, ''):
                errs.append(f'{name}: {k} is missing')
        for k in m:
            if k not in FIELDS:
                errs.append(f'{name}: unknown field {k!r} (allowed: {", ".join(FIELDS)})')
        n = m.get('id')
        if not isinstance(n, int):
            errs.append(f'{name}: id must be a number')
        else:
            if name[:4].isdigit() and int(name[:4]) != n:
                errs.append(f'{name}: id {n} does not match the file number {name[:4]}')
            if n in by_id:
                errs.append(f'{name}: id {n} is also used by {by_id[n][0].name}')
            by_id[n] = (p, m)
        if m.get('status') not in STATUSES:
            errs.append(f'{name}: status {m.get("status")!r} is not one of {", ".join(STATUSES)}')
        d, r = _date(m.get('date')), _date(m.get('recorded'))
        if m.get('date') and not d:
            errs.append(f'{name}: date {m.get("date")!r} is not YYYY-MM-DD')
        if m.get('recorded') and not r:
            errs.append(f'{name}: recorded {m.get("recorded")!r} is not YYYY-MM-DD')
        if d and r and d > r:
            errs.append(f'{name}: date {d} is after recorded {r}')
        for k in ('tags', 'supersedes', 'amends', 'amended_by', 'sources'):
            if m.get(k) is not None and not isinstance(m[k], list):
                errs.append(f'{name}: {k} must be a [list] (use [] for none)')
        for t in m.get('tags') or []:
            if not KEBAB.match(str(t)):
                errs.append(f'{name}: tag {t!r} must be lowercase kebab-case')
        for k in ('supersedes', 'amends', 'amended_by'):
            for s in m.get(k) or []:
                if not isinstance(s, int):
                    errs.append(f'{name}: {k} must list record numbers, got {s!r}')
        sb = m.get('superseded_by')
        if sb is not None and not isinstance(sb, int):
            errs.append(f'{name}: superseded_by must be a record number or null')
        if (m.get('status') == 'superseded') != (sb is not None):
            errs.append(f'{name}: status superseded and superseded_by go together')
        first = next((ln for ln in body.splitlines() if ln.strip()), '')
        if m.get('title') and first.strip() != f'# {m["title"]}':
            errs.append(f'{name}: the first line after the frontmatter must be "# {m["title"]}"')

    ids = sorted(by_id)
    if ids and ids != list(range(1, len(ids) + 1)):
        gaps = sorted(set(range(1, ids[-1] + 1)) - set(ids))
        errs.append(f'numbers must run 1..{len(ids)} without gaps; missing: {", ".join(f"{g:04d}" for g in gaps)}')

    for n, (p, m) in by_id.items():                  # supersede links point both ways
        for s in m.get('supersedes') or []:
            if not isinstance(s, int):
                continue
            if s not in by_id:
                errs.append(f'{p.name}: supersedes {s:04d}, which does not exist')
            elif by_id[s][1].get('superseded_by') != n:
                errs.append(f'{by_id[s][0].name}: superseded by {n:04d} ({p.name}), so it needs superseded_by: {n}')
        sb = m.get('superseded_by')
        if isinstance(sb, int):
            if sb not in by_id:
                errs.append(f'{p.name}: superseded_by {sb:04d}, which does not exist')
            elif n not in (by_id[sb][1].get('supersedes') or []):
                errs.append(f'{by_id[sb][0].name}: {p.name} says it is superseded by {sb:04d}, '
                            f'so this record needs {n} in supersedes')
        for s in m.get('amends') or []:                # amend links point both ways too
            if not isinstance(s, int):
                continue
            if s not in by_id:
                errs.append(f'{p.name}: amends {s:04d}, which does not exist')
            elif n not in (by_id[s][1].get('amended_by') or []):
                errs.append(f'{by_id[s][0].name}: amended by {n:04d} ({p.name}), so it needs {n} in amended_by')
        for a in m.get('amended_by') or []:
            if not isinstance(a, int):
                continue
            if a not in by_id:
                errs.append(f'{p.name}: amended_by {a:04d}, which does not exist')
            elif n not in (by_id[a][1].get('amends') or []):
                errs.append(f'{by_id[a][0].name}: {p.name} says it is amended by {a:04d}, '
                            f'so this record needs {n} in amends')

    known = {paths.DECISIONS_INDEX.name, paths.DECISIONS_TEMPLATE.name} | {p.name for p, *_ in recs}
    for p in sorted(paths.DECISIONS.glob('*.md')):
        if p.name not in known:
            errs.append(f'{p.name}: not a record; records are named NNNN-kebab-title.md')

    if not paths.DECISIONS_INDEX.exists():
        errs.append(f'{paths.DECISIONS_INDEX.name} is missing')
    else:
        text = paths.DECISIONS_INDEX.read_text(encoding='utf-8').replace('\r\n', '\n')
        try:
            if _with_index(text, recs) != text:
                errs.append(f'{paths.DECISIONS_INDEX.name}: the index is stale; run python docs/decisions/adr.py index')
        except ValueError as e:
            errs.append(str(e))
    return errs


# ── commands ────────────────────────────────────────────────────────────────
def slug(title, limit=60):
    s = re.sub(r'[^a-z0-9]+', '-', title.lower()).strip('-')
    if len(s) > limit:
        s = s[:limit].rsplit('-', 1)[0]
    return s


def cmd_new(a):
    recs = records()
    bad = [p.name for p, _, _, err in recs if err]
    if bad:
        sys.exit(f'fix these first: {", ".join(bad)}')
    n = max((m['id'] for _, m, _, _ in recs if isinstance(m.get('id'), int)), default=0) + 1
    today = dt.date.today().isoformat()
    tags = [t.strip() for t in (a.tags or '').split(',') if t.strip()]
    for t in tags:
        if not KEBAB.match(t):
            sys.exit(f'tag {t!r} must be lowercase kebab-case')
    if a.date and not _date(a.date):
        sys.exit(f'--date {a.date!r} is not YYYY-MM-DD')
    text = paths.DECISIONS_TEMPLATE.read_text(encoding='utf-8')
    text = text.replace('title: {{title}}', f'title: {_scalar(a.title)}')
    for k, v in {'id': str(n), 'title': a.title, 'date': a.date or today,
                 'recorded': today, 'tags': ', '.join(tags)}.items():
        text = text.replace('{{' + k + '}}', v)
    out = paths.DECISIONS / f'{n:04d}-{slug(a.title)}.md'
    out.write_text(text, encoding='utf-8', newline='\n')
    write_index()
    print(out.relative_to(paths.ROOT).as_posix())


def cmd_index(a):
    print('index updated' if write_index() else 'index already current')


def cmd_check(a):
    errs = problems(records())
    for e in errs:
        print(f'  {e}', file=sys.stderr)
    if errs:
        sys.exit(f'adr check: {len(errs)} problem(s) in {paths.DECISIONS.relative_to(paths.ROOT).as_posix()}/')
    print(f'adr check: {len(records())} record(s) ok')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    sub = ap.add_subparsers(dest='cmd', required=True)
    n = sub.add_parser('new', help='start a record from TEMPLATE.md (status: proposed)')
    n.add_argument('title')
    n.add_argument('--tags', help='comma-separated, kebab-case')
    n.add_argument('--date', help='when it was decided (YYYY-MM-DD); default today')
    n.set_defaults(fn=cmd_new)
    sub.add_parser('index', help='rewrite the table in docs/decisions/README.md').set_defaults(fn=cmd_index)
    sub.add_parser('check', help='validate every record and the index').set_defaults(fn=cmd_check)
    a = ap.parse_args()
    a.fn(a)


if __name__ == '__main__':
    main()
