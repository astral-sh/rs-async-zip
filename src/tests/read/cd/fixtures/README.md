# ZIP reader fixtures

These fixtures are generated independently of Malo with Python's standard-library
[`zipfile`](https://docs.python.org/3/library/zipfile.html). The Rust tests read the
checked-in bytes directly; Python is needed only to regenerate or check them.

```sh
uv run src/tests/read/cd/fixtures/generate.py
uv run src/tests/read/cd/fixtures/generate.py --check
```

The script's PEP 723 metadata declares Python 3.11+ with no third-party dependencies;
`uv` selects a compatible interpreter. Timestamps, entry order, permissions, and
compression settings are fixed. Deflate uses level zero to avoid dependence on
compression heuristics. `--output-dir PATH` writes or checks another directory.
`--check` fails for missing, changed, or unexpected ZIP files without changing them.

`zipfile` writes the local headers, payloads, descriptors, directories, and end
records. An unseekable output selects descriptors; `force_zip64=True` selects
64-bit entry sizes. The empty ZIP64 control temporarily lowers zipfile's entry
count threshold. Explicit byte edits in the generator create layouts that the
writer cannot produce, such as missing descriptors and inconsistent indexes.
No Malo fixture is read or modified by the generator.

| Fixtures | Purpose |
| --- | --- |
| `stored.zip`, `zip64.zip`, `empty.zip`, `empty-zip64.zip` | Ordinary Stored, ZIP64, and empty archive controls. |
| `nested-payload.zip` | A ZIP is valid entry data. The 2 MiB Stored payload also detects an extra payload scan. |
| `prefix.zip`, `concatenated.zip`, `concatenated-large.zip` | A self-consistent selected archive starts after byte zero; the large case puts the earlier footer outside the end-search window. |
| `zip-in-comment.zip`, `empty-in-comment.zip` | A complete later archive is embedded in an earlier archive's comment. |
| `concatenated-from-zero.zip` | The selected index points to byte zero but leaves earlier records in a gap before its directory. |
| `gap-before-directory.zip` | One unindexed byte separates the declared payload from the directory. |
| `local-size-overflow.zip`, `local-size-overlap.zip` | A local size overflows the range calculation or crosses into the directory. |
| `subdir.zip`, `subdir-reordered.zip` | Directory entries are valid, including when directory order differs from local-header order. |
| `descriptor-{stored,deflate,deflate-zip64}-{signed,unsigned}.zip` | Valid 32- and 64-bit descriptor spans, with and without signatures. |
| `descriptor-*-missing.zip` | An entry advertises a descriptor but has no space for it. |
| `descriptor-index-{missing,conflict}.zip` | A missing local-header reference or duplicate references with conflicting compressed lengths. |
| `empty-with-suffix.zip` | Existing trailing-content rejection still applies to empty archives. |
| `{stored,zip64}-padding-{4096,4097}.zip`, `{stored,zip64}-nonzero-suffix.zip` | Existing suffix tests cover the NUL-padding cap and nonzero data after padding without modifying Malo inputs. |
| `deflate-with-junk.zip` | The declared compressed span includes one byte after Deflate EOF; opening succeeds but entry completion rejects it. |
| `stored-truncated.zip` | The last payload byte is absent. The test supplies metadata previously read from `stored.zip` to model a source truncated after archive construction. |
