# Vendored xml5ever

`third_party/xml5ever/` is a copy of the `xml5ever` crate, the XML parser from the
[html5ever](https://github.com/servo/html5ever) repository. Armature builds against this copy instead of the crates.io
release, so that it can fix how XML documents are parsed into the DOM. This file records where the copy came from and
every change Armature makes to it, so that upstream changes can be brought in, and our changes carried forward,
without guesswork. It follows the same rules as [the vendored html5ever](html5ever.md) and
[the vendored Stylo](stylo.md).

## Upstream

| | |
|---|---|
| Upstream repository | https://github.com/servo/html5ever (the `xml5ever/` directory) |
| Imported release | `xml5ever` 0.40.0 from crates.io, built from upstream commit `a193ea7f2492d1e51eb32955a09c241345348dba` ("Compare prefix and local name when looking for duplicate attributes (#780)", tag `xml5ever-v0.40.0`) |
| Import commit | The Armature commit whose message has the trailer `Xml5ever-Import: a193ea7f2492d1e51eb32955a09c241345348dba` |

The import commit contains the crates.io package unmodified (without the `.cargo-ok` marker and the
`Cargo.toml.orig` that `.gitignore` leaves out), so `git diff <import commit> -- third_party/xml5ever` always shows
exactly what Armature has changed. The package's `src/` is identical to `xml5ever/src/` at the upstream commit. Its
`Cargo.toml` is the self-contained manifest that crates.io generates.

`html5ever`, `xml5ever` and `markup5ever` (see [the vendored markup5ever](markup5ever.md)) are vendored. Their other
sibling crates (`web_atoms`, `tendril`) still come from crates.io. The copy depends on `markup5ever` by version, and
the root `Cargo.toml` patches that version to the vendored copy. The XML serializer in
`src/serialize/` is no longer used by Servo, which has its own (`components/script/dom/xml/xmlserialization.rs`).

## How Servo uses the copy

- The root `Cargo.toml` points the `xml5ever` workspace dependency at `third_party/xml5ever` with a path dependency,
  and lists `third_party/xml5ever` in the workspace's `exclude` list, so the copy builds as its own package and
  `./mach fmt` and Servo's lints leave it alone.
- `Cargo.lock` still pins `xml5ever`'s dependencies; the import only removed its `source` and `checksum` lines.

## Rules for changing the copy

1. **One change, one entry.** Every change to `third_party/xml5ever/` belongs to an entry in [Changes](#changes)
   below, with an ID of the form `AX-NNN` (Armature xml5ever). Add the entry in the same commit as the change.
2. **Keep xml5ever commits separate.** A commit that touches `third_party/xml5ever/` may also touch this file, and
   nothing else. Make Servo-side changes (DOM, tests, expectations) in separate commits, so that every xml5ever change
   can be exported as a plain patch against upstream.
3. **Name the change in the commit.** Start the subject with `xml5ever:` and add an `Xml5ever-Change: AX-NNN`
   trailer.
4. **List every file.** Each entry's *Files* line lists every file it changes, as a path relative to
   `third_party/xml5ever/`. If a later change touches a file again, add the file to that change's entry too.
5. **Mark the code.** Put a `// Armature (AX-NNN): ...` comment at each changed spot, so the change is easy to find
   when resolving merge conflicts.
6. **Prefer small, additive diffs.** Change upstream lines only where needed, and do not restructure upstream code.

`./mach test-tidy` enforces rules 1 to 4 in the same way as for Stylo and html5ever.

## Bringing in a newer upstream xml5ever

1. Export Armature's changes against the current import:
   `git diff <import commit> HEAD -- third_party/xml5ever > /tmp/armature-xml5ever.diff`
2. Replace the copy with the new release, unmodified, from the crates.io package (for example the directory that
   `cargo fetch` unpacks under `~/.cargo/registry/src/`), leaving out `.cargo-ok`. Update the requirements of the
   sibling crates in `Cargo.toml` if the release needs newer ones.
3. Update the *Imported release* row above (the upstream commit is in the package's `.cargo_vcs_info.json`) and commit
   with the trailers `Xml5ever-Upstream: https://github.com/servo/html5ever` and `Xml5ever-Import: <commit>`. That
   commit becomes the new baseline.
4. Re-apply the changes from the repository root: `git apply --3way /tmp/armature-xml5ever.diff` (or entry by entry,
   using the commits found with `git log --grep "Xml5ever-Change: AX-NNN"`). Drop entries that upstream has made
   unnecessary, and say so under the entry. Commit the re-applied changes with their `Xml5ever-Change` trailers.
5. Build, run crown and `./mach test-tidy`, and rerun the XML tests (`domparsing`, `dom/nodes` `.xhtml` and `.svg`
   tests, `svg`) and the directories the entries name.

## Sending a change upstream

Each xml5ever commit touches only the copy (and this file), and the copy's `src/` matches the upstream
`xml5ever/src/`, so it can be exported as a patch that applies to the `xml5ever/` directory of a checkout of
`servo/html5ever`:

```sh
git format-patch --relative=third_party/xml5ever <import commit>..HEAD -- third_party/xml5ever
```

Apply it there with `git am --directory=xml5ever`. Note that Servo's AI policy also covers the `servo/html5ever`
repository.

## Changes

Each entry has this form (the check reads the `### AX-NNN` headings and the `Files` lines):

```markdown
### AX-001: Short summary
- Files: `src/tree_builder/mod.rs`
- Why: What the change enables or fixes, and which WPT tests it affects.
- Upstream: Whether the change could go upstream, or why it is Armature-only.
```

### AX-001: Keep namespace declaration attributes, in source order
- Files: `src/tokenizer/mod.rs`, `src/tree_builder/mod.rs`
- Why: The DOM keeps namespace declarations as attributes in the XMLNS namespace (`xmlns="..."` with no prefix and
  `xmlns:p="..."` with the prefix `xmlns` and the local name `p`), but the tree builder used them to bind names and
  then dropped them, and the tokenizer moved them to the front of the attribute list in reverse order. Keep them in
  the XMLNS namespace and leave every attribute in source order. Without them, `getAttribute("xmlns")`,
  `attributes` and the XML serialization of parsed documents lost their declarations. Affects
  domparsing/XMLSerializer-serializeToString.html and other tests that parse XML with namespace declarations.
- Upstream: Could go upstream as is.

### AX-002: Parse CDATA sections into CDATA section nodes
- Files: `src/tokenizer/interface.rs`, `src/tokenizer/mod.rs`, `src/tree_builder/mod.rs`, `src/tree_builder/types.rs`, `examples/simple_xml_tokenizer.rs`
- Why: The tokenizer emitted the contents of a CDATA section as characters, so the tree builder merged them into the
  surrounding text, and an empty CDATA section disappeared. The tokenizer now collects each section and emits it as a
  `CdataSection` token (keeping trailing `]` characters at the end of the input), and the tree builder appends it with
  `TreeSink::append_cdata_section` (markup5ever AM-001). Affects domparsing/DOMParser-parseFromString-xml-CDATA.html,
  domparsing/DOMParser-parseFromString-xml-empty-nodes.html and domparsing/xml-parse-serialize-roundtrip.html.
- Upstream: Could go upstream together with markup5ever AM-001. Adding a `Token` variant breaks code that matches
  tokens exhaustively, such as the `simple_xml_tokenizer` example, which this change updates.
