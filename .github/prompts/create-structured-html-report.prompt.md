# Create Structured HTML Report

Use the `structured-html-report-skill`.

Create a complete, single-file HTML report from the provided material.

Requirements:

- Use Tailwind CSS via CDN.
- Use semantic HTML.
- Use stable section IDs.
- Use bounded edit regions for all major sections.
- Use stable component IDs and `data-component` attributes.
- Use inline SVG for at least one meaningful visual model when appropriate.
- Use Chart.js only if actual chart data is present or requested.
- Include responsive layout.
- Include print CSS.
- Include a machine-readable `report-manifest` JSON block before `</body>`.
- Do not generate Markdown wrapped in HTML.
- Do not invent facts, dates, metrics, or source claims.

The report should be visually polished, but also easy to edit iteratively in future Copilot interactions.
