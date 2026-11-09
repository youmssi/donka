# WCAG 2.2 AA self-assessment

**Standard:** Web Content Accessibility Guidelines (WCAG) 2.2, level AA. **Assessed:**
2026-11-09, Studio web app on `develop` at `4ce8625`. **Status:** self-assessed (DNK-73);
no external audit. Live browser journeys run EN/FR, light/dark, desktop and 390 px on
every change; they check behaviour, not every success criterion. Limits are written below,
not hidden.

## Summary by principle

| Principle | Level A | Level AA | Notes |
| --- | --- | --- | --- |
| 1 Perceivable | Met, with limits | Partial | Text alternatives, contrast and resize below |
| 2 Operable | Met | Met, with limits | Full keyboard operation is a design rule; target sizes below |
| 3 Understandable | Met | Met | EN/FR catalogues, consistent navigation, form errors in words |
| 4 Robust | Met | Met | Semantic HTML/React; assistive-tech matrix is the limit |

## Where Studio stands, criterion families

- **Text alternatives (1.1):** icons carry labels; screenshots and charts have text
  equivalents or adjacent data. Decorative marks are hidden from assistive technology.
- **Adaptable (1.3):** semantic landmarks, headings and form labels throughout; reading
  order follows visual order; no information by colour alone (statuses pair colour with
  text in EN/FR).
- **Distinguishable (1.4):** the light and dark themes are checked at 390 px and desktop in
  the live matrix. **Limit:** AA contrast (4.5:1) is verified by design tokens, not by an
  automated contrast gate in CI; a token change could regress it silently. Gap owner:
  maintainer; an automated check is a backlog candidate.
- **Keyboard (2.1):** every journey is operable by keyboard; shared forms stay disabled
  until hydrated so focus cannot land on inert controls; autofocus is restored after load.
- **Focus (2.4):** visible focus ring from the UI kit; skip behaviour follows the kit.
  **Limit:** the JDM graph editor is canvas-like; node editing has keyboard paths through
  the inspector, but free-form graph navigation by keyboard alone remains hard. Accepted
  for the pilot; stated here so a bank can plan staffing.
- **Target size (2.5.8, AA new in 2.2):** primary controls meet 24×24 CSS px through the
  UI kit; dense table row actions are the known exception class and are being aligned.
  Status: **Partial**.
- **Input modalities (2.5):** no path-based gestures are required; dragging in the graph
  editor always has a form-based alternative (inspector fields).
- **Readable (3.1):** `lang` is set per locale (EN/FR); catalogues keep both languages at
  key parity (checked in CI).
- **Predictable (3.2):** no context change on focus or input; navigation is consistent.
- **Input assistance (3.3):** errors are sentences, not codes, in the user's language;
  required fields and formats are stated before submission; sensitive submissions (setup,
  approvals) summarise before confirming. The new 2.2 AA criterion 3.3.8 (accessible
  authentication) is met by design: no cognitive test; password managers and paste work,
  and the second step offers authenticator apps and mailed codes.
- **Compatible (4.1):** status messages use live regions in the kit components; names,
  roles and values come from Radix primitives.

## Not claimed

- No VPAT/ACR is issued; this page is the statement a bank's accessibility team reviews.
- Native mobile apps: none exist. The mobile web at 390 px is the assessed surface.
- Documents the bank exports (PDF statements) come from the bank's own tools, not from
  Studio; CSV exports carry plain text.

When a criterion regresses in a journey, the journey fails in CI; when a limit above is
lifted, this page changes in the same commit.
