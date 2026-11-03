# Vendored Stylo

`third_party/stylo/` is a copy of [Stylo](https://github.com/servo/stylo), the CSS engine that Servo shares with
Firefox. Armature builds against this copy instead of a Git dependency, so that it can enable and fix CSS features
that the upstream copy builds only for Gecko. This file records where the copy came from and every change Armature
makes to it, so that upstream changes can be brought in, and our changes carried forward, without guesswork.

## Upstream

| | |
|---|---|
| Upstream repository | https://github.com/servo/stylo (itself synced from mozilla-central's `servo/` directory) |
| Imported revision | `cd230184a620c3e6c0f25faa07838fddcefec32d` ("Enable `background-clip: text` behind a pref (#501)", 2026-10-29) |
| Import commit | The Armature commit whose message has the trailer `Stylo-Import: cd230184a620c3e6c0f25faa07838fddcefec32d` |

The import commit contains the upstream tree unmodified, so `git diff <import commit> -- third_party/stylo` always
shows exactly what Armature has changed.

## How Servo uses the copy

- The root `Cargo.toml` points the eight Stylo crates (`selectors`, `servo_arc`, `stylo`, `stylo_atoms`, `stylo_dom`,
  `stylo_malloc_size_of`, `stylo_static_prefs`, `stylo_traits`) at `third_party/stylo/<crate>` with path dependencies.
- `third_party/stylo` is in the workspace's `exclude` list. Stylo keeps its own `Cargo.toml` workspace manifest, which
  its crates inherit from, so the copy builds exactly as it does upstream and its own files need no changes.
- `Cargo.lock` still pins every external dependency of Stylo; the import only removed the Git `source` lines.
- Tidy, taplo and ruff already ignore `third_party/`, and `./mach fmt` formats only workspace members, so neither
  reformats the copy.

## Rules for changing the copy

1. **One change, one entry.** Every change to `third_party/stylo/` belongs to an entry in [Changes](#changes) below,
   with an ID of the form `AS-NNN` (Armature Stylo). Add the entry in the same commit as the change.
2. **Keep Stylo commits separate.** A commit that touches `third_party/stylo/` may also touch this file, and nothing
   else. Make Servo-side changes (prefs, layout, tests, expectations) in separate commits. This keeps every Stylo
   change exportable as a plain patch against upstream Stylo.
3. **Name the change in the commit.** Start the subject with `stylo:` and add a `Stylo-Change: AS-NNN` trailer.
4. **List every file.** Each entry's *Files* line lists every file it changes, as a path relative to
   `third_party/stylo/`. If a later change touches a file again, add the file to that change's entry too.
5. **Mark the code.** Put a `// Armature (AS-NNN): ...` comment (or `# Armature (AS-NNN): ...` in Python, TOML and
   Mako) at each changed spot, so the change is easy to find when resolving merge conflicts.
6. **Prefer small, additive diffs.** Change upstream lines only where needed, and prefer switching a property from
   `engine = "gecko"` to both engines (or behind a `servo_pref`) over restructuring code, so upstream merges stay easy.

`./mach test-tidy` enforces rules 1 to 4: it lists the files that differ from the import commit and checks that each
is named in an entry, that every entry's files really differ, and that every commit since the import that touches the
copy has a valid `Stylo-Change` trailer and touches nothing outside `third_party/stylo/` and this file.

## Bringing in a newer upstream Stylo

1. Export Armature's changes against the current import:
   `git diff <import commit> HEAD -- third_party/stylo > /tmp/armature-stylo.diff`
2. Replace the copy with the new upstream revision, unmodified:
   `rm -r third_party/stylo && mkdir third_party/stylo && git -C <stylo clone> archive <new rev> | tar -x -C third_party/stylo`
3. Update the *Imported revision* row above and commit with the trailers `Stylo-Upstream: https://github.com/servo/stylo`
   and `Stylo-Import: <new rev>`. That commit becomes the new baseline.
4. Re-apply the changes from the repository root: `git apply --3way /tmp/armature-stylo.diff` (or entry by entry,
   using the commits found with `git log --grep "Stylo-Change: AS-NNN"`). Drop entries that upstream has made
   unnecessary, and say so under the entry. Commit the re-applied changes with their `Stylo-Change` trailers.
5. Build, run crown and `./mach test-tidy`, and rerun the CSS test directories.

## Sending a change upstream

Because each Stylo commit touches only the copy (and this file), it can be exported as a patch that applies to a
checkout of `servo/stylo`:

```sh
git format-patch --relative=third_party/stylo <import commit>..HEAD -- third_party/stylo
```

Note that Servo's AI policy also covers the `servo/stylo` repository.

## Changes

Each entry has this form (the check reads the `### AS-NNN` headings and the `Files` lines):

```markdown
### AS-001: Short summary
- Files: `style/properties/longhands.toml`, `style/values/specified/box.rs`
- Why: What the change enables or fixes, and which WPT tests it affects.
- Upstream: Whether the change could go upstream, or why it is Armature-only.
```

### AS-401: Enable the CSS Shapes properties for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `shape-outside`, `shape-margin` and `shape-image-threshold` for Servo behind the new preference
  `layout.css.shapes.enabled`, so they parse, compute, serialize and interpolate. Servo's float layout does not wrap
  content around the shapes yet and ignores the values. Affects `css/css-shapes/parsing`, `css/css-shapes/animation`
  and `css/css-shapes/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-402: Enable the motion path properties and the offset shorthand for Servo
- Files: `style/properties/longhands.toml`, `style/properties/shorthands.toml`, `style/properties/shorthands.rs`
- Why: Builds `offset-position`, `offset-distance`, `offset-rotate`, `offset-anchor` and the `offset` shorthand for
  Servo behind `layout.css.motion-path.enabled`, the preference that already gates `offset-path`, and builds the
  shorthand's parser and serializer for Servo. Servo's layout does not apply motion paths yet. Affects
  `css/motion/parsing`, `css/motion/animation` and `css/motion/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-403: Enable transform-box for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`, `style/values/specified/transform.rs`
- Why: Builds `transform-box` for Servo behind the new preference `layout.css.transform-box.enabled`, and lets Servo
  parse all five keywords, since Servo's layout now uses the content box or the border box as the reference box.
  Affects `css/css-transforms/parsing/transform-box-*.html`, `css/css-transforms/transform-box/cssbox-*.html` and
  `css/css-transforms/inheritance.html`.
- Upstream: Could go upstream together with Servo's layout support, but Servo's AI policy currently prevents sending
  it.

### AS-404: Enable text-decoration-skip-ink for Servo behind a preference
- Files: `style/properties/longhands.toml`, `stylo_static_prefs/preferences.toml`
- Why: Builds `text-decoration-skip-ink` for Servo behind the new preference
  `layout.css.text-decoration-skip-ink.enabled`. Servo's painting ignores the value. Affects
  `css/css-text-decor/parsing/text-decoration-skip-ink-*.html`, `css/css-text-decor/text-decoration-skip-ink.html`
  and `css/css-text-decor/inheritance.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-405: Enable hyphenate-limit-chars for Servo behind the hyphens preference
- Files: `style/properties/longhands.toml`
- Why: Builds `hyphenate-limit-chars` for Servo behind `layout.css.hyphens.enabled`, which already gates `hyphens`
  and `hyphenate-character`. Servo's layout ignores the value. Affects
  `css/css-text/parsing/hyphenate-limit-chars-*.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.

### AS-406: Enable math-shift for Servo behind the math properties preference
- Files: `style/properties/longhands.toml`
- Why: Builds `math-shift` for Servo behind `layout.css.math-properties.enabled`, which already gates `math-depth`
  and `math-style`. Servo's layout ignores the value. Affects
  `mathml/relations/css-styling/parsing/math-shift-*.html`.
- Upstream: Could go upstream as is, but Servo's AI policy currently prevents sending it.
