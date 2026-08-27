#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///

"""Regenerate the ZIP reader fixtures, or verify their bytes with --check."""

from __future__ import annotations

import argparse
import io
from pathlib import Path
import struct
import sys
import zipfile


class Unseekable(io.BytesIO):
    """Make zipfile write a data descriptor instead of patching the local header."""

    def seek(self, *args):
        raise io.UnsupportedOperation("fixture output is not seekable")


def archive(
    entries=(("entry", b"hello"),),
    *,
    prefix=b"",
    descriptor=False,
    zip64=False,
    compression=zipfile.ZIP_STORED,
):
    output = Unseekable() if descriptor else io.BytesIO()
    output.write(prefix)
    with zipfile.ZipFile(output, "w") as writer:
        for name, payload in entries:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (0o40755 << 16 | 0x10) if name.endswith("/") else 0o100644 << 16
            info.compress_type = compression
            # This spelling also works before Python 3.13's public compress_level property.
            # Level zero avoids dependence on a particular zlib compression strategy.
            info._compresslevel = 0
            with writer.open(info, "w", force_zip64=zip64) as entry:
                entry.write(payload)
    data = bytearray(output.getvalue())
    # Prefixes and concatenated archives still have a self-consistent selected directory.
    with zipfile.ZipFile(io.BytesIO(data)) as reader:
        assert [(entry.filename, reader.read(entry)) for entry in reader.infolist()] == list(entries)
    return data


def end_offset(data):
    offset = len(data) - 22
    assert data[offset : offset + 4] == b"PK\x05\x06"
    return offset


def read_u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0]


def write_u32(data, offset, value):
    struct.pack_into("<I", data, offset, value)


def directory_offset(data):
    return read_u32(data, end_offset(data) + 16)


def descriptor_offset(data):
    name_length, extra_length = struct.unpack_from("<HH", data, 26)
    offset = 30 + name_length + extra_length + read_u32(data, directory_offset(data) + 20)
    assert data[offset : offset + 4] == b"PK\x07\x08"
    return offset


def remove_descriptor_bytes(data, count):
    data = data.copy()
    start = descriptor_offset(data)
    directory = directory_offset(data)
    del data[start : start + count]
    write_u32(data, end_offset(data) + 16, directory - count)
    return data


def fixtures():
    stored = archive()
    result = {
        "stored.zip": stored,
        "empty.zip": archive(()),
        "prefix.zip": archive(prefix=b"junk"),
        "concatenated.zip": archive(prefix=stored),
        "nested-payload.zip": archive((("entry", stored.ljust(2 * 1024 * 1024, b"\0")),)),
        "concatenated-large.zip": archive((("entry", stored.ljust(131_072, b"\0")),), prefix=stored),
        "subdir.zip": archive((("dir/", b""), ("dir/entry", b"hello"))),
    }

    # Force zipfile to emit ZIP64 end records even though there are no entries.
    limit = zipfile.ZIP_FILECOUNT_LIMIT
    try:
        zipfile.ZIP_FILECOUNT_LIMIT = -1
        result["empty-zip64.zip"] = archive(())
        result["zip64.zip"] = archive(compression=zipfile.ZIP_DEFLATED)
    finally:
        zipfile.ZIP_FILECOUNT_LIMIT = limit
    result["empty-with-suffix.zip"] = result["empty.zip"] + b"\x01"
    for name in ["stored", "zip64"]:
        for padding in [4096, 4097]:
            result[f"{name}-padding-{padding}.zip"] = result[f"{name}.zip"] + bytes(padding)
        result[f"{name}-nonzero-suffix.zip"] = result[f"{name}.zip"] + b"\0\0X"

    # A later archive lives inside the first archive's comment. zipfile has already
    # written its absolute offsets; only the first EOCD's comment length changes.
    for name, entries in [("zip-in-comment", (("entry", b"hello"),)), ("empty-in-comment", ())]:
        data = archive(entries, prefix=stored)
        struct.pack_into("<H", data, end_offset(stored) + 20, len(data) - len(stored))
        result[f"{name}.zip"] = data

    # Point the selected directory at the first archive's entry. Its declared data
    # ends before the intervening directory/footer and second local entry.
    data = result["concatenated.zip"].copy()
    write_u32(data, directory_offset(data) + 42, 0)
    result["concatenated-from-zero.zip"] = data

    data = stored.copy()
    directory = directory_offset(data)
    data[directory:directory] = b"X"
    write_u32(data, end_offset(data) + 16, directory + 1)
    result["gap-before-directory.zip"] = data

    data = archive((("entry", b""),), zip64=True)
    extra = 30 + struct.unpack_from("<H", data, 26)[0]
    assert data[extra : extra + 4] == struct.pack("<HH", 1, 16)
    struct.pack_into("<Q", data, extra + 12, 2**64 - 1)
    directory = directory_offset(data)
    extra = directory + 46 + struct.unpack_from("<H", data, directory + 28)[0]
    data[extra:extra] = struct.pack("<HHQ", 1, 8, 2**64 - 1)
    struct.pack_into("<H", data, directory + 30, 12)
    write_u32(data, directory + 20, 2**32 - 1)
    end = end_offset(data)
    write_u32(data, end + 12, read_u32(data, end + 12) + 12)
    result["local-size-overflow.zip"] = data

    data = archive((("entry", b""),))
    write_u32(data, 18, 1)  # The local data span includes the first byte of the directory.
    write_u32(data, directory_offset(data) + 20, 1)
    result["local-size-overlap.zip"] = data

    data = result["subdir.zip"].copy()
    directory, end = directory_offset(data), end_offset(data)
    name_length, extra_length, comment_length = struct.unpack_from("<HHH", data, directory + 28)
    split = directory + 46 + name_length + extra_length + comment_length
    data[directory:end] = data[split:end] + data[directory:split]
    result["subdir-reordered.zip"] = data

    for name, compression, zip64 in [
        ("stored", zipfile.ZIP_STORED, False),
        ("deflate", zipfile.ZIP_DEFLATED, False),
        ("deflate-zip64", zipfile.ZIP_DEFLATED, True),
    ]:
        data = archive(descriptor=True, compression=compression, zip64=zip64)
        result[f"descriptor-{name}-signed.zip"] = data
        result[f"descriptor-{name}-unsigned.zip"] = remove_descriptor_bytes(data, 4)
        result[f"descriptor-{name}-missing.zip"] = remove_descriptor_bytes(
            data, directory_offset(data) - descriptor_offset(data)
        )

    original = result["descriptor-deflate-signed.zip"]
    directory, end = directory_offset(original), end_offset(original)
    data = original.copy()
    write_u32(data, directory + 42, 1)  # The directory no longer identifies the local header.
    result["descriptor-index-missing.zip"] = data

    data = original.copy()
    data[end:end] = original[directory:end]
    write_u32(data, end + 20, read_u32(original, directory + 20) + 1)
    end = end_offset(data)
    struct.pack_into("<HH", data, end + 8, 2, 2)
    write_u32(data, end + 12, 2 * (end_offset(original) - directory))
    result["descriptor-index-conflict.zip"] = data

    # The declared data span includes junk after a complete Deflate stream.
    data = archive(compression=zipfile.ZIP_DEFLATED)
    directory = directory_offset(data)
    compressed = read_u32(data, directory + 20) + 1
    data[directory:directory] = b"X"
    write_u32(data, 18, compressed)
    write_u32(data, directory + 1 + 20, compressed)
    write_u32(data, end_offset(data) + 16, directory + 1)
    result["deflate-with-junk.zip"] = data
    # Used with the complete stored.zip directory to simulate a truncated source.
    result["stored-truncated.zip"] = stored[: directory_offset(stored) - 1]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare without writing any files")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    if sys.version_info < (3, 11):
        parser.error("Python 3.11 or newer is required for reproducible forced-ZIP64 headers")
    expected = fixtures()
    if not args.check:
        args.output_dir.mkdir(parents=True, exist_ok=True)
    mismatches = []
    for name, data in sorted(expected.items()):
        path = args.output_dir / name
        if not path.exists() or path.read_bytes() != data:
            if args.check:
                mismatches.append(name)
            else:
                path.write_bytes(data)
    if args.check:
        mismatches += sorted(path.name for path in args.output_dir.glob("*.zip") if path.name not in expected)
    if mismatches:
        parser.exit(1, "Missing, changed, or unexpected fixtures: " + ", ".join(mismatches) + "\n")
    print(f"{'Verified' if args.check else 'Generated'} {len(expected)} fixtures")


if __name__ == "__main__":
    main()
