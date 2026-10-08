# Donka brand

One family: a rounded tile carrying a simple line glyph, next to the lowercase **donka** wordmark
(Geist Bold, outlined) and the product name (Geist Regular). Every product keeps the tile, the
stroke and the corner radius, and has its own glyph and colour.

| Product | Mark | Colour | Glyph |
| --- | --- | --- | --- |
| Donka, Studio | ![](donka-mark.svg) | `#1d4f91` | A D holding an input that runs into a decision node |
| Runtime | ![](runtime-mark.svg) | `#0f766e` | The D with a forward chevron: decisions served |
| CLI | ![](cli-mark.svg) | `#1e293b` | A terminal prompt |
| Fieldkit | ![](fieldkit-mark.svg) | `#b45309` | A text field above a ticked checkbox |

## Files

- `<product>-mark.svg`: the tile alone, 64 × 64. Use it for app icons, favicons, avatars and
  anywhere space is tight; it stays legible at 16 px.
- `<product>-lockup-light.svg`, `<product>-lockup-dark.svg`: tile, wordmark and product name, for
  light and dark backgrounds. Use them in headers, documents and slides.

The text in lockups is outlined, so the files look the same on every machine.

## Rules

- Keep the tile's proportions and corner radius (15/64); never stretch, outline or recolour the
  glyph other than white on the tile, or the tile colour on white (as in the README banners).
- Leave at least a quarter of the tile's width clear around a mark or lockup.
- Studio's web app draws the mark inline (`BrandMark`, in `apps/web`), coloured by the theme's
  primary colour, so it follows light and dark mode.
- Banner navy is `#0b1f3a` to `#1d4f91` (README banners in `.github/assets/`).

Geist is licensed under the SIL Open Font License; outlining it in a logo is allowed.
