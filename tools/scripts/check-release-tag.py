"""Check a local release tag against HEAD, the workspace version and release notes.

No tags, commits or remotes are created. Used by the artifact-only release workflow.
"""
import argparse
import json
import pathlib
import re
import subprocess
import sys


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--tag', required=True)
    ap.add_argument('--github-output', type=pathlib.Path)
    args = ap.parse_args()
    if not re.fullmatch(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', args.tag):
        raise ValueError('Release tag must be vMAJOR.MINOR.PATCH (no leading zeroes).')
    repo = pathlib.Path(__file__).resolve().parents[2]

    def run(*command):
        return subprocess.check_output(command, cwd=repo, text=True).strip()

    if run('git', 'rev-parse', f'refs/tags/{args.tag}^{{}}') != run('git', 'rev-parse', 'HEAD'):
        raise ValueError(f'{args.tag} does not identify the checked-out commit.')
    meta = json.loads(run('cargo', 'metadata', '--locked', '--format-version', '1', '--no-deps'))
    version = next(p['version'] for p in meta['packages'] if p['name'] == 'td2192')
    if args.tag != f'v{version}':
        raise ValueError(f'Tag {args.tag} disagrees with workspace version {version}.')
    changelog = (repo / 'CHANGELOG.md').read_text(encoding='utf-8')
    if not re.search(rf'^## \[{re.escape(version)}\](?:\s.*)?$', changelog, re.M):
        raise ValueError(f'CHANGELOG.md has no section for {version}.')
    if args.github_output:
        with args.github_output.open('a', encoding='utf-8') as out:
            out.write(f'version={version}\n')
    print(f'{args.tag}: HEAD, workspace version and changelog agree.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'ERROR: {error}', file=sys.stderr)
        sys.exit(1)
