#!/usr/bin/env python3
"""Developer-run bounded allocation/depth/repeated-candidate checks; no downloads."""
import argparse
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile


def cases():
    return ([('depth', d, 1, 0, 1) for d in (1, 16, 128, 1000)]
            + [('keys', 8, k, 0, 1) for k in (16, 4096, 65536)]
            + [('scalar', 1, 1, n, 1) for n in (16, 65536, 1048576, 8388608)]
            + [('stream', 8, 16, 64, n) for n in (1, 100, 1000)])


def validate(row, depth, key, scalar, count, chunk, mode):
    expected = dict(depth=depth, key_bytes=key, scalar_bytes=scalar,
                    candidates=count, chunk_bytes=chunk, mode=mode,
                    input_bytes=(depth * (key + 5) + scalar + 2) * count,
                    completed=count, invalid=0, live_after_drop=0)
    for name, value in expected.items():
        if row[name] != value:
            raise ValueError(f'{name}: expected {value}, got {row[name]}')
    if row['retained_at_end'] != row['retained_after_first']:
        raise ValueError('pipeline retained heap grew after first candidate')
    if not (0 <= row['retained_at_end'] <= row['peak_live_bytes'] <= row['requested_bytes']):
        raise ValueError('inconsistent heap accounting')
    if row['output_bytes'] <= 0:
        raise ValueError('no formatted output')


def validate_comparisons(rows):
    # Repeated candidates allocate keys/spools again; total calls may grow.
    # Live scalar content should stream without a token-sized allocation.
    for mode in ('live', 'memory', 'spill'):
        for axis in ('stream',):
            values = {row['retained_at_end'] for row in rows
                      if row['mode'] == mode and row['axis'] == axis}
            if len(values) > 1:
                raise ValueError(f'{mode}: retained heap changes with candidate count')
    peaks = {row['peak_live_bytes'] for row in rows
             if row['mode'] == 'live' and row['axis'] == 'scalar'}
    if len(peaks) > 1:
        raise ValueError('live scalar heap changes with token size/chunk/repetition')
    # Counts agree across modes/chunks, but this is not a byte-equivalence test.
    groups = {}
    for row in rows:
        identity = tuple(row[k] for k in ('depth', 'key_bytes', 'scalar_bytes', 'candidates'))
        groups.setdefault(identity, set()).add(row['output_bytes'])
    if any(len(values) != 1 for values in groups.values()):
        raise ValueError('output counts disagree across modes/chunks/repetitions')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolchain-bin', type=Path, required=True)
    parser.add_argument('--repeats', type=int, default=3)
    args = parser.parse_args()
    if not 3 <= args.repeats <= 10:
        parser.error('repeats must be between 3 and 10')
    toolchain = args.toolchain_bin.resolve(strict=True)
    if any(not (toolchain / name).is_file() for name in ('rustc', 'cargo')):
        parser.error('toolchain directory must contain cargo and rustc')
    workspace = Path(__file__).resolve().parent.parent
    repo = workspace / 'repos/flon'
    target = repo / 'target'
    target.mkdir(exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='perf-resources-', dir=target))
    print(f'Artifacts: {directory}', flush=True)
    wrapper = workspace / 'scripts/with-rust'
    environment = dict(os.environ, LC_ALL='C', RUSTC=str(toolchain / 'rustc'),
                       CARGO_TARGET_DIR=str(target))
    common = runpy.run_path(str(workspace / 'scripts/measure-flon-matrix'))

    def run(label, command):
        command = list(map(str, command))
        (directory / f'{label}.command.json').write_text(json.dumps(command, indent=2) + '\n')
        with (directory / f'{label}.stdout').open('wb') as output, (directory / f'{label}.stderr').open('wb') as errors:
            result = subprocess.run(command, cwd=repo, env=environment, stdout=output, stderr=errors)
        (directory / f'{label}.exit').write_text(f'{result.returncode}\n')
        if result.returncode:
            raise RuntimeError(f'{label} failed; inspect {directory}')

    (directory / 'method.json').write_text(json.dumps({
        'repeats': args.repeats, 'toolchain_bin': str(toolchain),
        'metric': 'current-thread requested heap during parser+formatter lifetime; not RSS',
        'fixture': 'nested keyed objects around UTF-8/raw-escape string; plain counting writer',
        'flags': {key: value for key, value in environment.items() if key in (
            'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET',
            'CARGO_PROFILE_RELEASE_OPT_LEVEL', 'CARGO_PROFILE_RELEASE_LTO',
            'CARGO_PROFILE_RELEASE_CODEGEN_UNITS')},
        'timing': 'instrumented allocation checks; not a throughput benchmark',
    }, indent=2) + '\n')
    for label, command in [('host', ['uname', '-a']), ('compiler', [wrapper, toolchain / 'rustc', '-Vv']),
        ('revision', ['git', 'rev-parse', 'HEAD']), ('status', ['git', 'status', '--short']),
        ('diff', ['git', 'diff', 'HEAD', '--binary'])]:
        run(label, command)
    # Untracked example sources are not included in git diff; retain the actual inputs.
    for source in ('examples/resources.rs', 'examples/support/allocation.rs', 'Cargo.toml', 'Cargo.lock'):
        (directory / Path(source).name).write_bytes((repo / source).read_bytes())
    (directory / 'runner.py').write_bytes(Path(__file__).read_bytes())
    run('build', [wrapper, toolchain / 'cargo', 'build', '--offline', '--locked', '--release', '--example', 'resources'])
    binary = target / 'release/examples/resources'
    (directory / 'executable.json').write_text(json.dumps({'sha256': common['sha256'](binary)}) + '\n')
    rows = []
    for axis, depth, key, scalar, count in cases():
        for mode in ('live', 'memory', 'spill'):
            for chunk in (8192, 65536):
                for iteration in range(1, args.repeats + 1):
                    label = f'{axis}-{depth}-{key}-{scalar}-{count}-{mode}-{chunk}-{iteration}'
                    print(f'Checking {label}', flush=True)
                    run(label, [binary, depth, key, scalar, count, chunk, mode])
                    row = json.loads((directory / f'{label}.stdout').read_text())
                    validate(row, depth, key, scalar, count, chunk, mode)
                    rows.append(dict(axis=axis, iteration=iteration, **row))
                    (directory / 'results.json').write_text(json.dumps(rows, indent=2) + '\n')
    validate_comparisons(rows)
    print(f'All resource checks passed. Share {directory}/results.json', flush=True)


if __name__ == '__main__':
    main()
