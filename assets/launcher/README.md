# CLI launcher icons

`claude.svg` and `openai.svg` are unmodified Simple Icons 14.15.0 paths:

- https://github.com/simple-icons/simple-icons/blob/14.15.0/icons/claude.svg
- https://github.com/simple-icons/simple-icons/blob/14.15.0/icons/openai.svg

The OpenAI mark identifies Codex. The paths are CC0-1.0; the full license is
`licenses/simple-icons-LICENSE.md`. Brand names and marks remain their owners'
trademarks. `bolt.svg` is a project-authored geometric lightning symbol.

GPUI embeds these SVGs with `include_bytes!`; no runtime asset directory is needed
on Mac or Windows. Compose embeds the same 24 × 24 path data in
`app/src/jvmSharedMain/kotlin/kim/jujin/rovessh/ui/LauncherIcons.kt`.
Keep the SVG and Compose paths in sync. Both renderers tint the paths with the
current theme's foreground color.
