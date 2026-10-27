"""Compare compiler layout with a resym reference dump (no target execution).

Example: python scripts/validate_header.py --compiler <cl.exe> --reference raw.h
    --header repaired.h --output-dir validation [--c]
The reference must be an unmodified export of the same PDB. Bitfield positions
require separate tests; offsetof cannot be applied to a bitfield in C or C++.
"""
import argparse
import json
import pathlib
import re
import struct
import subprocess


def metadata(text):
    types, fields = [], []
    owner, depth = None, 0
    for line in text.splitlines():
        match = re.match(r'^(struct|union) (\w+) \{ /\* Size=(0x[0-9a-f]+) \*/', line)
        if match:
            owner, depth = match[2], 0
            types.append((owner, int(match[3], 16)))
        if owner:
            match = re.match(r'\s*/\* (0x[0-9a-f]+) \*/ (.+);$', line)
            if match and ':' not in match[2]:
                declaration = match[2]
                if '(' in declaration:
                    name = re.search(r'\(\*\s*(\w+)', declaration)
                    if not name and 'HalPowerSetRebootHandler' in declaration:
                        name = re.search(r'\b(HalPowerSetRebootHandler)\b', declaration)
                else:
                    name = re.search(r'(\w+)(?:\[\d+\])*$', declaration)
                if name:
                    fields.append((owner, name[1], int(match[1], 16)))
            depth += line.count('{') - line.count('}')
            if depth == 0:
                owner = None
    return types, fields


def check_bitfields(args, sizes):
    fields = []
    owner, depth = None, 0
    for line in args.reference.read_text().splitlines():
        match = re.match(r'^(struct|union) (\w+) \{', line)
        if match:
            owner, depth = match[2], 0
        if owner:
            match = re.match(r'\s*/\* (0x[0-9a-f]+) \*/ .+?\b(\w+) : (\d+); /\* BitPos=(\d+) \*/$', line)
            if match and int(match[3]) and match[2] not in {'char', 'short', 'int', 'long'}:
                fields.append((owner, match[2], int(match[1], 16), int(match[3]), int(match[4])))
            depth += line.count('{') - line.count('}')
            if depth == 0:
                owner = None
    source, obj = args.output_dir / 'bitfields.c', args.output_dir / 'bitfields.obj'
    source.write_text('#include "' + args.header.resolve().as_posix() + '"\n' + '\n'.join(
        t + ' resym_bits_' + str(i) + ' = { .' + member + ' = -1 };'
        for i, (t, member, _, _, _) in enumerate(fields)) + '\n')
    result = subprocess.run([args.compiler, '/nologo', '/W0', '/TC', '/std:c11', '/c',
                             '/Fo' + str(obj.resolve()), str(source.resolve())], capture_output=True, text=True)
    (args.output_dir / 'bitfields-compiler.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        print(result.stdout + result.stderr)
        raise SystemExit(result.returncode)
    data = obj.read_bytes()
    _, count, _, table, symbols, optional, _ = struct.unpack_from('<HHLLLHH', data)
    sections = []
    for i in range(count):
        offset = 20 + optional + i * 40
        size, raw = struct.unpack_from('<LL', data, offset + 16)
        sections.append(data[raw:raw + size])
    strings = table + symbols * 18
    actual = {}
    i = 0
    while i < symbols:
        offset = table + i * 18
        name = data[offset:offset + 8]
        if name[:4] == b'\0' * 4:
            location = strings + struct.unpack_from('<L', name, 4)[0]
            name = data[location:].split(b'\0', 1)[0]
        else:
            name = name.rstrip(b'\0')
        value, section = struct.unpack_from('<Lh', data, offset + 8)
        if name.startswith(b'resym_bits_') and section > 0:
            index = int(name[len(b'resym_bits_'):])
            actual[index] = sections[section - 1][value:value + sizes[fields[index][0]]]
        i += 1 + data[offset + 17]
    assert len(actual) == len(fields)
    mismatches = []
    for i, (t, member, offset, width, position) in enumerate(fields):
        expected = (((1 << width) - 1) << (offset * 8 + position)).to_bytes(sizes[t], 'little')
        if actual[i] != expected:
            mismatches.append(dict(name=t + '.' + member, kind='bitfield',
                                   expected=expected.hex(), actual=actual[i].hex()))
    return len(fields), mismatches


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True)
    parser.add_argument('--reference', type=pathlib.Path, required=True)
    parser.add_argument('--header', type=pathlib.Path, required=True)
    parser.add_argument('--output-dir', type=pathlib.Path, required=True)
    parser.add_argument('--c', action='store_true')
    parser.add_argument('--bitfields', action='store_true', help='Also compare compiler-initialized bitfield masks (requires --c)')
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    types, fields = metadata(args.reference.read_text())
    expressions = [('sizeof(' + name + ')', size, name, 'size') for name, size in types]
    expressions += [('((unsigned long long)&(((' + name + '*)0)->' + field + '))',
                     offset, name + '.' + field, 'offset') for name, field, offset in fields]
    source = args.output_dir / ('layout.c' if args.c else 'layout.cpp')
    obj = args.output_dir / 'layout.obj'
    source.write_text('#include "' + args.header.resolve().as_posix() + '"\n' +
                      'unsigned long long resym_layout[] = {\n' +
                      ',\n'.join(item[0] for item in expressions) + '\n};\n')
    command = [args.compiler, '/nologo', '/TC' if args.c else '/TP',
               '/std:c11' if args.c else '/std:c++17', '/c', '/Fo' + str(obj.resolve()), str(source.resolve())]
    result = subprocess.run(command, capture_output=True, text=True)
    (args.output_dir / 'compiler.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        print(result.stdout + result.stderr)
        raise SystemExit(result.returncode)
    data = obj.read_bytes()
    machine, sections, _, _, _, optional, _ = struct.unpack_from('<HHLLLHH', data)
    assert machine == 0x8664, 'Validation expects an x64 COFF object'
    values = None
    for i in range(sections):
        offset = 20 + optional + 40 * i
        if data[offset:offset + 8].rstrip(b'\0') == b'.data':
            size, raw = struct.unpack_from('<LL', data, offset + 16)
            values = data[raw:raw + size]
    assert values is not None and len(values) == 8 * len(expressions)
    mismatches = []
    for i, (_, expected, name, kind) in enumerate(expressions):
        actual = struct.unpack_from('<Q', values, i * 8)[0]
        if actual != expected:
            mismatches.append(dict(name=name, kind=kind, expected=expected, actual=actual))
    bitfields = 0
    if args.bitfields:
        assert args.c, '--bitfields requires a C-compatible header and --c'
        bitfields, bit_mismatches = check_bitfields(args, dict(types))
        mismatches.extend(bit_mismatches)
    report = dict(types=len(types), member_offsets=len(fields), bitfields=bitfields, mismatches=mismatches)
    (args.output_dir / 'results.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(types=len(types), member_offsets=len(fields), bitfields=bitfields,
                          mismatches=len(mismatches), examples=mismatches[:20]), indent=2))
    raise SystemExit(bool(mismatches))


if __name__ == '__main__':
    main()
