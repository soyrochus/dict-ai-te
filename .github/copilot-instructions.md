# GitHub Copilot instructions: structured HTML reports

When the user asks for reports, executive summaries, visual briefings, RFI/RFP analysis, architecture assessments, delivery status reports, transformation plans, roadmaps, comparison documents, or similar artifacts, do not default to plain Markdown if the user wants a polished or graphical output.

Use the `structured-html-report-skill` when available.

Default report output should be a complete, single-file HTML document using:

- Tailwind CSS via CDN.
- Semantic HTML.
- Stable section IDs.
- Bounded edit regions.
- Stable component IDs.
- Inline SVG for diagrams.
- Optional Chart.js only when real charts are needed.
- Minimal external dependencies.
- Print-friendly CSS.
- Responsive layout.
- A machine-readable report manifest.

The report must be visually structured, not Markdown wrapped in HTML.

The report must also be iteratively editable. Optimize the HTML for future AI-assisted modification, not only for immediate visual rendering.

When editing an existing structured HTML report:

- Preserve the design system unless explicitly asked to change it.
- Preserve existing IDs.
- Replace the smallest bounded edit region that satisfies the request.
- Avoid rewriting unrelated sections.
- Update the manifest if sections or components are added, removed, renamed, or substantially changed.
- Do not rename IDs unless necessary.
- Do not use React, Vue, Angular, Svelte, build tools, external image assets, or heavy dependencies unless explicitly requested.
