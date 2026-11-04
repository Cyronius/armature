# Vendored html5ever

`third_party/html5ever/` is a copy of the `html5ever` crate, the HTML parser from the
[html5ever](https://github.com/servo/html5ever) repository. Armature builds against this copy instead of the crates.io
release, so that it can fix parsing rules that upstream has not implemented yet. This file records where the copy came
from and every change Armature makes to it, so that upstream changes can be brought in, and our changes carried
forward, without guesswork. It follows the same rules as [the vendored Stylo](stylo.md).

## Upstream

| | |
|---|---|
| Upstream repository | https://github.com/servo/html5ever (the `html5ever/` directory) |
| Imported release | `html5ever` 0.40.1 from crates.io, built from upstream commit `d7232d746d3112f08926d169591576bd12fe2fdf` ("Bump version number to 0.40.1 (#786)", tag `html5ever-v0.40.1`) |
| Import commit | The Armature commit whose message has the trailer `Html5ever-Import: d7232d746d3112f08926d169591576bd12fe2fdf` |

The import commit contains the crates.io package unmodified (without the `.cargo-ok` marker), so
`git diff <import commit> -- third_party/html5ever` always shows exactly what Armature has changed. The package's
`src/` is identical to `html5ever/src/` at the upstream commit. Its `Cargo.toml` is the self-contained manifest that
crates.io generates; the upstream one (`Cargo.toml.orig`) inherits from the upstream workspace.

Only `html5ever` and `xml5ever` (see [the vendored xml5ever](xml5ever.md)) are vendored. Their sibling crates
(`markup5ever`, `web_atoms`, `tendril`) still come from crates.io, and the copy depends on the same `markup5ever`
release as the rest of Servo.

## How Servo uses the copy

- The root `Cargo.toml` points the `html5ever` workspace dependency at `third_party/html5ever` with a path
  dependency, and lists `third_party/html5ever` in the workspace's `exclude` list, so the copy builds as its own
  package and `./mach fmt` and Servo's lints leave it alone.
- `Cargo.lock` still pins `html5ever`'s dependencies; the import only removed its `source` and `checksum` lines.

## Rules for changing the copy

1. **One change, one entry.** Every change to `third_party/html5ever/` belongs to an entry in [Changes](#changes)
   below, with an ID of the form `AH-NNN` (Armature html5ever). Add the entry in the same commit as the change.
2. **Keep html5ever commits separate.** A commit that touches `third_party/html5ever/` may also touch this file, and
   nothing else. Make Servo-side changes (DOM, tests, expectations) in separate commits, so that every html5ever change
   can be exported as a plain patch against upstream.
3. **Name the change in the commit.** Start the subject with `html5ever:` and add an `Html5ever-Change: AH-NNN`
   trailer.
4. **List every file.** Each entry's *Files* line lists every file it changes, as a path relative to
   `third_party/html5ever/`. If a later change touches a file again, add the file to that change's entry too.
5. **Mark the code.** Put a `// Armature (AH-NNN): ...` comment at each changed spot, so the change is easy to find
   when resolving merge conflicts.
6. **Prefer small, additive diffs.** Change upstream lines only where needed, and do not restructure upstream code.

`./mach test-tidy` enforces rules 1 to 4 in the same way as for Stylo.

## Bringing in a newer upstream html5ever

1. Export Armature's changes against the current import:
   `git diff <import commit> HEAD -- third_party/html5ever > /tmp/armature-html5ever.diff`
2. Replace the copy with the new release, unmodified, from the crates.io package (for example the directory that
   `cargo fetch` unpacks under `~/.cargo/registry/src/`), leaving out `.cargo-ok`. Update the `html5ever` requirement
   of the sibling crates in `Cargo.toml` if the release needs newer ones.
3. Update the *Imported release* row above (the upstream commit is in the package's `.cargo_vcs_info.json`) and commit
   with the trailers `Html5ever-Upstream: https://github.com/servo/html5ever` and `Html5ever-Import: <commit>`. That
   commit becomes the new baseline.
4. Re-apply the changes from the repository root: `git apply --3way /tmp/armature-html5ever.diff` (or entry by entry,
   using the commits found with `git log --grep "Html5ever-Change: AH-NNN"`). Drop entries that upstream has made
   unnecessary, and say so under the entry. Commit the re-applied changes with their `Html5ever-Change` trailers.
5. Build, run crown and `./mach test-tidy`, and rerun the HTML parsing tests (`html/syntax`, `html/semantics`,
   `domparsing`) and the directories the entries name.

## Sending a change upstream

Each html5ever commit touches only the copy (and this file), and the copy's `src/` matches the upstream
`html5ever/src/`, so it can be exported as a patch that applies to the `html5ever/` directory of a checkout of
`servo/html5ever`:

```sh
git format-patch --relative=third_party/html5ever <import commit>..HEAD -- third_party/html5ever
```

Apply it there with `git am --directory=html5ever`. Note that Servo's AI policy also covers the `servo/html5ever`
repository.

## Changes

Each entry has this form (the check reads the `### AH-NNN` headings and the `Files` lines):

```markdown
### AH-001: Short summary
- Files: `src/tree_builder/rules.rs`
- Why: What the change enables or fixes, and which WPT tests it affects.
- Upstream: Whether the change could go upstream, or why it is Armature-only.
```

### AH-001: Report the end tags of SVG `script` elements
- Files: `src/tree_builder/mod.rs`, `src/tree_builder/rules.rs`
- Why: Implements the foreign content rules for "an end tag whose tag name is "script", if the current node is an
  SVG `script` element" and for a self-closing `<script/>` start tag in the SVG namespace (upstream FIXMEs,
  servo/html5ever#118): the tree builder pops the element and returns it to the sink as a script to run, as it does
  for HTML scripts, so the parser pauses and Servo prepares and runs it. Without them, SVG `script` elements inside
  HTML documents (inline `<svg>`) never ran. Affects svg/scripted,
  trusted-types/script-enforcement-*, html/semantics/scripting-1 and other tests that put a `script` inside `<svg>`.
- Upstream: Could go upstream as is.

### AH-002: Tell the sink about every element popped off the stack of open elements
- Files: `src/tree_builder/mod.rs`, `src/tree_builder/rules.rs`
- Why: Several HTML and SVG features act when an element "is popped off the stack of open elements" (the
  sink's `pop`), but upstream only calls `TreeSink::pop` from some of the places that pop elements. Others remove
  elements by truncating the stack (an end tag in foreign content, `pop_until`, `pop_until_current`, the end tag of
  a generic element in body, the adoption agency's step 10 and the `frameset` start tag), or never push them
  (self-closing foreign elements such as `<svg/>`). This change adds a `pop_to_len` helper and calls `sink.pop`
  for each of those elements, current node first. Servo fires the `load` event of `svg` elements from the sink's
  `pop`, so without this, `<svg onload>` in HTML documents never fired. Elements that the adoption agency removes
  from the middle of the stack, and HTML void elements, are unchanged. Affects svg/animations,
  svg/render/reftests and other HTML tests that wait for the `load` event of an `svg` element.
- Upstream: Could go upstream as is.

### AH-003: Give the `xmlns` attribute of foreign elements no prefix
- Files: `src/tree_builder/mod.rs`
- Why: The "adjust foreign attributes" table maps `xmlns` to no prefix, the XMLNS namespace and the local name
  `xmlns`, but upstream built that name with `qualname!("" xmlns "xmlns")`, which gives it the empty prefix instead of
  none. `svg.attributes[0].prefix` was then `""` and its `name` `":xmlns"`, and the XML serializer, which tells
  default namespace declarations from prefix declarations by their prefix, wrote `<svg xmlns="...">` as
  `<xmlns:svg ...>`, so inline SVG with an `xmlns` attribute stopped rendering. Affects svg/render, css/filter-effects
  and other reftests that draw inline `<svg xmlns="http://www.w3.org/2000/svg">`, and XMLSerializer output for
  inline SVG and MathML.
- Upstream: Could go upstream as is.
