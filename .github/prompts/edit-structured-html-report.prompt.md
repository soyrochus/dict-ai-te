# Edit Structured HTML Report

Use the `structured-html-report-skill`.

Modify the existing HTML report according to the user's request.

Editing rules:

- Preserve the existing design system unless explicitly asked to change it.
- Preserve stable IDs.
- Replace the smallest bounded edit region that satisfies the request.
- Do not rewrite unrelated sections.
- Do not rename IDs unless necessary.
- Update the report manifest if sections or components are added, removed, renamed, or substantially changed.
- If changing a table, edit only the relevant row when possible.
- If changing an SVG diagram, edit only the relevant SVG group when possible.
- If changing a chart, update the chart data JSON block before changing rendering logic.
- Return the complete updated HTML file if file editing is available.
- Otherwise return the exact replacement block, including the opening and closing edit-region comments.
