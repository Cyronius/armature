# Armature: HTML

Detail behind the HTML row of the [README](../../README.md) conformance table: what Armature changed and what is still missing. Other areas are listed in [the index](README.md).

## What has been done

- **HTML reflecting attributes**: about 130 missing IDL attributes (table, table cell, row and column, heading and
  `body` colors, `ol`/`li`/`dl`, `embed`/`object`/`param`/`area`, frames, `marquee`, form controls, `meta.media`,
  `a.download`/`ping`, `enterKeyHint`, `inputMode` and more). The 11 automated `html/dom/reflection-*` files now
  pass all 61,302 subtests, down from about 5,300 failures. Table `span`, `colSpan` and `rowSpan` keep the assigned
  content attribute while clamping the reflected value, fixing 38 subtests. HTML floating-point reflection now
  parses numeric prefixes after ASCII whitespace and serializes exponent values as JavaScript does; `textarea`
  positive-value fallback, negative `pre.width`, and empty `track.kind` reflection close the remaining 47 failures
  in the seven-file baseline. The enumerated ARIA attributes (`ariaBusy`, `ariaLive`, ...) are behind the
  `dom_aria_enumerated_reflection_enabled` preference.
- **Fetch priority reflection**: `fetchPriority` reflects `fetchpriority` on HTML `img`, `link` and `script`, and SVG
  `image` and `script` elements. All 10 focused subtests now pass across five files.
- **[Popover API](https://html.spec.whatwg.org/multipage/popover.html)**, behind the `dom_popover_enabled`
  preference (off by default; enabled for the tests): the `popover` attribute (`auto`, `manual` and `hint`),
  `showPopover()`/`hidePopover()`/`togglePopover()`, `beforetoggle`/`toggle` events, `popovertarget` on buttons and
  inputs, button `commandForElement` reflection and activation, light dismiss, nested popovers, and dialog and
  fullscreen integration. Open popovers now join sequential focus navigation at their invoker, including across
  slots and with explicit tabindex values. The fallback `<summary>` in a `<details>` without an explicit summary
  now participates in focus navigation, and WebDriver keys sent to an active shadow host reach its focused
  descendant. Twelve more focus subtests pass. An open popover used as the `source` of another popover now remains its
  ancestor, preserving auto and hint stacks; six more subtests pass. Canceled form resets no longer block later
  resets, empty labels inside invoker buttons allow button activation, and pointer events on disabled controls now
  light-dismiss open popovers; six more subtests pass. Light dismiss now ignores right clicks and touch scrolls, and
  leaves a popover open if its press target is removed; three more subtests pass. Activation now uses the original
  click target across shadow boundaries, so a nested popover stays open when clicked; one more subtest passes.
  A displayed popover with an otherwise generic accessibility role now maps to `group`, and its role updates when
  display or the `popover` attribute changes; two more subtests pass. `html/semantics/popovers` has 25 failures in
  the latest run, down from 3,902. Pressing Escape now also closes
  dialogs and close watchers.
- **`HTMLElement.draggable`** (323 subtests), **fractional `MouseEvent` coordinates** as CSSOM View specifies (433),
  and the current HTML rules for **valid custom element names** (113). Element names with code points above U+10FFF,
  such as most emoji, are no longer rejected.
- **Vendored html5ever**: the HTML parser is vendored in `third_party/html5ever/` with a change log
  (`third_party/html5ever.md`), checked by `./mach test-tidy` like the Stylo one. Its changes implement the
  parsing rules for SVG `script` end tags and self-closing `<script/>` in SVG, which upstream had not implemented,
  tell Servo about every element the parser closes, and give the `xmlns` attribute of inline SVG and MathML no
  prefix (it had the empty prefix, so its `name` was `":xmlns"`).
- **Processing instructions in HTML**: the HTML parser now parses `<?target data?>` into a `ProcessingInstruction`
  node, following the HTML Standard's new tokenizer states (it used to make a bogus comment; `<?xml ...>`,
  `<?xml-stylesheet ...>` and invalid targets still do). This fixes all 83 `processing-instructions` html5lib tree
  construction subtests in each of the three html5lib test files, 12 more html5lib subtests, the 20 parser subtests of
  the `ProcessingInstruction` attributes test, and 52 Sanitizer API subtests. The XML parser no longer creates a
  `ProcessingInstruction` node for the XML declaration (`<?xml version="1.0"?>`).
- **XML serialization**: `XMLSerializer`, `innerHTML` and `outerHTML` in XML documents, and the drawing of inline SVG
  now use an implementation of the [DOM Parsing](https://w3c.github.io/DOM-Parsing/#xml-serialization) XML
  serialization algorithm instead of xml5ever's serializer. It keeps namespaces with prefixes and `xmlns`
  declarations as the spec says, escapes attribute values (`<`, `>`, `"`, tabs and newlines), writes doctypes with
  their public and system IDs, and makes `innerHTML`/`outerHTML` throw `InvalidStateError` for markup that would not
  be well-formed. 48 failing subtests and 14 reftests now pass in `domparsing`, `html/syntax`, the
  `ProcessingInstruction` tests and the SVG tests, among them inline SVG `<image>` elements with `data:` URLs that
  contain `<`. Inline SVG `<use>` also follows `xlink:href`. `XMLHttpRequest.send()` with an XML document uses it
  too (it used the HTML serializer), which fixes 4 `xhr` subtests.
- **Vendored xml5ever**: the XML parser is vendored in `third_party/xml5ever/` with a change log
  (`third_party/xml5ever.md`), checked by `./mach test-tidy` like the other vendored copies. Parsed XML documents now
  keep their namespace declarations (`xmlns`, `xmlns:p`) as attributes, in source order, which fixes 10 more
  `XMLSerializer` subtests and 7 `Node.lookupPrefix()` subtests.
- **Vendored markup5ever and CDATA sections**: markup5ever, which holds the interface (`TreeSink`) through which
  both parsers build the DOM, is vendored in `third_party/markup5ever/` with its own change log
  (`third_party/markup5ever.md`). A new `TreeSink` method lets the XML parser create CDATA sections, which it used to
  turn into text: `DOMParser` now produces `CDATASection` nodes (also empty ones), and CDATA sections survive a parse
  and serialize round trip, which fixes 14 `domparsing` subtests.
- **Images in embedded elements**: `<embed src>` and `<object data>` resolve image URLs and use the image cache's
  natural dimensions for `object-fit` and `object-position`. In the 85-file CSS Images reftest run, 81 tests newly
  pass; eight related `object-position` reftests also pass. Four tentative `contain-intrinsic-size` cases still fail.

- **Canvas shadows, layers and filters** (22 November 2026): shapes, text and images now cast the shadow that
  `shadowOffsetX`, `shadowOffsetY`, `shadowBlur` and `shadowColor` describe (all 105 shadow tests that failed now
  pass); previously shapes and text cast none, and a filled or stroked rectangle or image lost its own pixels when
  it did. `beginLayer()`, `endLayer()` and the `filter` attribute are implemented for `CanvasRenderingContext2D` and
  `OffscreenCanvasRenderingContext2D`: a layer is drawn on its own target, which extends beyond the canvas so that
  blurs and shadows can bring in what is drawn outside it, and is composited with the filter, shadow, alpha and
  composite operation set when it began. The filter functions of CSS (`blur()`, `drop-shadow()`, `brightness()`
  and the other color functions) and the `gaussianBlur` and `dropShadow` primitives of `beginLayer()` are
  supported; blurs use the algorithm of resvg so that canvases and SVG filters agree. About 100 more canvas layer
  and filter tests pass.
- **The `dir` attribute and `:dir()`**: Stylo parses `:dir()` (change AS-537); the DOM keeps the state that it
  matches on up to date as `dir` attributes, text and the tree change; and `dir=auto`, `<bdi>` without a `dir`,
  and text controls with `dir=auto` resolve their direction from the first strong character (`input` and
  `textarea` from their value, other elements from their descendants' text, skipping `bdi`, `script`, `style`,
  `textarea` and elements with their own `dir`). 97 tests of `html/dom/elements/global-attributes` and
  `css/selectors` newly pass.
- **Body margins**: `marginwidth`, `leftmargin`, `marginheight` and `topmargin` on `<body>` set its margins
  (16 reftests). The margins that a `frame` or `iframe` gives to its document's body are not mapped.
- **Lazy loading of images**: `img.loading` exists, and `loading=lazy` images (and `srcdoc` iframes) wait to be
  fetched until they come within 1250px of the viewport or of a scroller, using an intersection observer that
  the document keeps for them; they no longer delay the window `load` event. About 20 `the-img-element` tests newly
  pass. Images in nested documents and iframes with a `src` are still loaded right away.
- **`link rel=expect`**: a `link rel=expect blocking=render` that points into its own document blocks rendering until
  the parser has finished the element with that id (or an element with that id or an anchor with that name is
  inserted), or until parsing finishes. About 35 render-blocking subtests newly pass; unblocking by removing the
  element or the `blocking` attribute of scripts and styles is still missing.
- **Preload scanner**: the scanner that fetches resources ahead of a blocked parser now also scans input written by
  `document.write()`, skips templates, MathML, `nomodule` and unsupported-type scripts, disabled or media-never-matching
  stylesheets and images that a meta CSP forbids, and fetches `link rel=preload`, video posters, SVG images and scripts
  by `href`, `srcset` and `picture` sources. About 60 speculative-parsing tests newly pass.
- **`showPicker()`** on `input` and `select` (validation, cross-origin and activation checks; no picker is shown), and
  MathML is enabled for the HTML IDL harness.
- **Form controls and selectors**: `change` fires when a text control that the user edited loses focus; `:in-range`
  parses (Stylo change AS-538) and `:in-range`, `:out-of-range`, `:required` and `:optional` match; `:read-write` covers
  editable content and not hidden inputs; `:link` no longer matches `link` elements; clones of inputs get their own
  validity; `valueAsDate` is null for `datetime-local`; stepping decimal values no longer shows float noise.
- **Lazy iframes with a `src`** are now deferred until near the viewport, like images and `srcdoc` iframes; framesets
  are laid out as grids (frames still don't load documents).
- **Smaller HTML APIs**: `BarProp` and the bar properties of `Window`; `history.scrollRestoration`, fully-active checks
  for `history.go()` and `pushState()` with an empty URL; `spellcheck`, `autocorrect`, `autocapitalize` and
  `writingSuggestions`; `hidden` as boolean, number or `until-found`; names of `embed` and `object` elements as
  properties of the document; lowercase autofill values; email values sanitized again when `multiple` changes.
- **Declarative partial updates**: `<template for>` replaces the range between `<?start>` and `<?end>` (or at a
  `<?marker>`) while parsing, streaming or with `buffer`, in documents and in fragments; `htmlFor` and `buffer` on
  `HTMLTemplateElement`. The streaming `*HTMLUnsafe` methods are missing.
- **Navigation details**: `popstate` on fragment navigation, `pagereveal`, `window.originAgentCluster` (the getter only,
  without agent cluster isolation), URLs of `base` and `history` parsed with the document's encoding, and the query
  encoding tests enabled.
- **Canvas fonts in workers**: `FontFace` and `FontFaceSet` (`self.fonts`) are exposed to workers, and an
  `OffscreenCanvas` in a worker resolves its `font` with a style system of its own, against the default font.
- **Conic gradients** and `BarProp.visible` for popups.
- **hidden=until-found**: `content-visibility: hidden` skips its contents, navigating to a fragment inside it fires
  `beforematch` and reveals it (and closed `details` ancestors), and nothing in it can be focused.
- **Opener policies**: a popup whose `Cross-Origin-Opener-Policy` does not match the one of its opener loses its
  opener, starts with no name and looks closed to the window that opened it, and a frame of another origin than a
  `same-origin` top-level document cannot give its popups an opener. The policies are kept in a table that the script
  threads of one process share, so this does not work between processes, the popup stays in the opener's group of
  browsing contexts for the constellation, and neither reporting nor `noopener-allow-popups` are implemented.
- **Embedder policy**: the `Cross-Origin-Embedder-Policy` header is parsed for documents and workers (with
  `credentialless`), `crossOriginEmbedderPolicy` reflects it, and `crossOriginIsolated` exists but is always false
  because opener policies are not implemented.
- **Smaller fixes**: empty file controls in form data, lazy iframes keep the URL they were deferred with, and
  `autocapitalize` and `autocorrect` come from the form owner.
- **Navigating other windows**: `location.href` and `replace()` work on windows of other origins (through a new
  constellation message), a sandboxed document can only navigate itself, its descendants, the windows it opened and,
  with `allow-top-navigation`, the top-level window, nested frames are unloaded (rather than their parent again), and
  `pageshow` bubbles and is cancelable.
- **Focusgroups**: directional keys move the focus between the items of a `focusgroup` (behavior tokens with their
  default axes and wrapping, `inline`, `block`, `wrap`, `nowrap`, `nomemory`, `focusgroupstart`, opt-out with `none`,
  and grids made of tables with the wrap and flow tokens). The Tab key stops once per segment of a group, and
  `focusGroup` and `focusGroupStart` reflect the attributes. Reading flow, shadow trees, feeds and the accessibility
  roles are missing.
- **Canvas text drawing styles**: `fontKerning`, `fontStretch`, `fontVariantCaps`, `textRendering`, `letterSpacing`,
  `wordSpacing` and `lang`. Kerning, the spacings and the language affect shaping; the other styles are stored.

### Drag and drop of draggable elements

`components/script/drag/element_drag.rs` implements the drag-and-drop processing model for draggable elements
inside one document. Pressing the primary button on an element whose `draggable` state is true (an `img` or an `a`
with `href` is draggable by default) arms a drag; once the pointer has moved 3 pixels with the button down the
`dragstart` event fires at the element, and when the default is not prevented the drag starts and takes over the
mouse until it ends:

- Each move fires `drag` at the source, `dragenter` at a new target (followed by `dragleave` at the old one) and
  `dragover` at the target. The `dropEffect` of the shared `DataTransfer` starts from the default for the
  `effectAllowed` (copy for `uninitialized` and `all`, the only effect for `copy`, `link` and `move`, copy for
  `copyLink` and `copyMove`, move for `linkMove`) and the current drag operation is its value after a cancelled
  event, if the `effectAllowed` permits it, or `none`.
- Releasing the button fires `drop` at the target when it accepted the drag with an operation other than `none`,
  and `dragleave` otherwise, then `dragend` at the source with `dropEffect` set to the operation.
- While the drag lasts the mouse events (`mousemove` and `mouseup`) are not sent to the page. The data store is
  read/write in `dragstart`, read-only in `drop` and protected otherwise.
- A press on a text input, `textarea`, `select` or editable element (or inside one) never starts a drag of the
  element around it.

Drags of the selection, of text and of links that start a drag without a `draggable` element, and drags to and from
other documents or the operating system, are not implemented. `DataTransfer.effectAllowed` used to write to
`dropEffect`; the setter now sets the right value.

## What has not been done

- Four subtests (two in `XMLSerializer-serializeToString.html`, two doctype subtests in `xml-serialization.xhtml`)
  expect behaviour that the DOM Parsing spec changed in August 2026 and that Armature follows: a doctype ID that
  contains `"` is quoted with `'`, and an attribute keeps its own prefix when the element does not declare that
  prefix itself (an open web-platform-tests change updates the tests). One more expects a redundant `xmlns=""` to be
  kept, which the spec drops.
- 25 popover subtests still fail in the latest run: layout has no real top layer or `::backdrop` (open popovers are
  drawn as `position: fixed`), some focus navigation cases remain incomplete, four inactive-document cases expect
  listeners to run despite the DOM Standard's fully active document check, and 6 other tests contradict the spec.
- The tentative ARIA reflection test still has 30 failing subtests.
- Canvas layers and filters: the `CanvasFilter` primitives other than `gaussianBlur` and `dropShadow`
  (`colorMatrix`, `componentTransfer`, `convolveMatrix`, `turbulence`) are ignored, `url()` filters are not
  supported, layers whose content is moved by more than 2,048 pixels by a filter or shadow are clipped, and
  `drawImage()`, `createPattern()`, `putImageData()` and `toDataURL()` do not throw while a layer is open. The
  composite-operation grids that combine filters and shadows still fail.
- `dir=auto` does not follow changes to the value of text controls, and slots do not contribute their assigned
  nodes to the auto direction of their ancestors.
