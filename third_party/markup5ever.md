# Vendored markup5ever

`third_party/markup5ever/` is a copy of the `markup5ever` crate, the code that html5ever and xml5ever share (most
importantly the `TreeSink` trait, through which both parsers build the DOM), from the
[html5ever](https://github.com/servo/html5ever) repository. Armature builds against this copy instead of the crates.io
release, so that it can extend the interface between the parsers and Servo's DOM. This file records where the copy
came from and every change Armature makes to it, so that upstream changes can be brought in, and our changes carried
forward, without guesswork. It follows the same rules as [the vendored html5ever](html5ever.md),
[the vendored xml5ever](xml5ever.md) and [the vendored Stylo](stylo.md).

## Upstream

| | |
|---|---|
| Upstream repository | https://github.com/servo/html5ever (the `markup5ever/` directory) |
| Imported release | `markup5ever` 0.40.0 from crates.io, built from upstream commit `a193ea7f2492d1e51eb32955a09c241345348dba` ("Compare prefix and local name when looking for duplicate attributes (#780)") |
| Import commit | The Armature commit whose message has the trailer `Markup5ever-Import: a193ea7f2492d1e51eb32955a09c241345348dba` |

The import commit contains the crates.io package unmodified (without the `.cargo-ok` marker and the
`Cargo.toml.orig` that `.gitignore` leaves out), so `git diff <import commit> -- third_party/markup5ever` always
shows exactly what Armature has changed. The package's sources (`lib.rs`, `interface/`, `util/`, `serialize.rs`) are
identical to the upstream `markup5ever/` directory at the upstream commit. Its `Cargo.toml` is the self-contained
manifest that crates.io generates.

`html5ever`, `xml5ever` and `markup5ever` are vendored. Their other sibling crates (`web_atoms`, `tendril`) still come
from crates.io.

## How Servo uses the copy

- The root `Cargo.toml` points the `markup5ever` workspace dependency at `third_party/markup5ever` with a path
  dependency, and lists `third_party/markup5ever` in the workspace's `exclude` list, so the copy builds as its own
  package and `./mach fmt` and Servo's lints leave it alone.
- The vendored html5ever and xml5ever depend on `markup5ever` by version, as published. The root `Cargo.toml` replaces
  that crates.io release with the copy in its `[patch.crates-io]` section, so that every crate in the build uses the
  same copy. Keep the copy's version equal to the version they ask for, or Cargo ignores the patch.
- `Cargo.lock` still pins `markup5ever`'s dependencies; the import only removed its `source` and `checksum` lines.

## Rules for changing the copy

1. **One change, one entry.** Every change to `third_party/markup5ever/` belongs to an entry in [Changes](#changes)
   below, with an ID of the form `AM-NNN` (Armature markup5ever). Add the entry in the same commit as the change.
2. **Keep markup5ever commits separate.** A commit that touches `third_party/markup5ever/` may also touch this file,
   and nothing else. Make parser changes (in the vendored html5ever or xml5ever) and Servo-side changes in separate
   commits, so that every markup5ever change can be exported as a plain patch against upstream.
3. **Name the change in the commit.** Start the subject with `markup5ever:` and add a `Markup5ever-Change: AM-NNN`
   trailer.
4. **List every file.** Each entry's *Files* line lists every file it changes, as a path relative to
   `third_party/markup5ever/`. If a later change touches a file again, add the file to that change's entry too.
5. **Mark the code.** Put a `// Armature (AM-NNN): ...` comment at each changed spot, so the change is easy to find
   when resolving merge conflicts.
6. **Prefer small, additive diffs.** Change upstream lines only where needed, and do not restructure upstream code.
   New `TreeSink` methods get a default implementation that keeps the old behavior, so that other sinks keep
   building.

`./mach test-tidy` enforces rules 1 to 4 in the same way as for Stylo, html5ever and xml5ever.

## Bringing in a newer upstream markup5ever

1. Export Armature's changes against the current import:
   `git diff <import commit> HEAD -- third_party/markup5ever > /tmp/armature-markup5ever.diff`
2. Replace the copy with the new release, unmodified, from the crates.io package (for example the directory that
   `cargo fetch` unpacks under `~/.cargo/registry/src/`), leaving out `.cargo-ok`. Bring in the html5ever and
   xml5ever releases built from the same upstream commit at the same time, since they require a matching
   `markup5ever` version.
3. Update the *Imported release* row above (the upstream commit is in the package's `.cargo_vcs_info.json`) and commit
   with the trailers `Markup5ever-Upstream: https://github.com/servo/html5ever` and `Markup5ever-Import: <commit>`.
   That commit becomes the new baseline.
4. Re-apply the changes from the repository root: `git apply --3way /tmp/armature-markup5ever.diff` (or entry by
   entry, using the commits found with `git log --grep "Markup5ever-Change: AM-NNN"`). Drop entries that upstream has
   made unnecessary, and say so under the entry. Commit the re-applied changes with their `Markup5ever-Change`
   trailers.
5. Build, run crown and `./mach test-tidy`, and rerun the parser tests (`html/syntax`, `domparsing`, `dom/nodes`) and
   the directories the entries name.

## Sending a change upstream

Each markup5ever commit touches only the copy (and this file), and the copy's sources match the upstream
`markup5ever/` directory, so it can be exported as a patch that applies to that directory of a checkout of
`servo/html5ever`:

```sh
git format-patch --relative=third_party/markup5ever <import commit>..HEAD -- third_party/markup5ever
```

Apply it there with `git am --directory=markup5ever`. Note that Servo's AI policy also covers the `servo/html5ever`
repository.

## Changes

Each entry has this form (the check reads the `### AM-NNN` headings and the `Files` lines):

```markdown
### AM-001: Short summary
- Files: `interface/tree_builder.rs`
- Why: What the change enables or fixes, and which WPT tests it affects.
- Upstream: Whether the change could go upstream, or why it is Armature-only.
```
