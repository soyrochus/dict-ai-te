---
name: structured-html-report-skill
description: Generate rich, self-contained, iteratively editable HTML reports using Tailwind CSS, semantic structure, bounded edit regions, stable IDs, inline SVG, optional Chart.js, print support, and a machine-readable manifest. Use this skill whenever the user asks for a visual report, polished report, executive briefing, architecture assessment, delivery status, RFI/RFP analysis, roadmap, proposal, or other artifact where Markdown is too limited.
---

# Structured HTML Report Skill

## Purpose

Use this skill to create rich HTML-based reports instead of plain Markdown.

The output should be a visually polished, browser-openable `.html` document. It should be suitable for executive readers, technical stakeholders, delivery managers, architects, clients, or internal teams depending on the user's request.

The report must be more than decorated Markdown. It should use layout, hierarchy, cards, tables, callouts, diagrams, metrics, visual summaries, optional charts, and clear sections to make the information easier to understand.

The report is also an editable document artifact. Optimize the HTML for future AI-assisted modification, not only for immediate visual rendering.

## Core requirements

Unless the user requests otherwise, produce a complete single-file HTML document:

- Include `<!doctype html>`.
- Include `<html lang="...">`.
- Include a complete `<head>`.
- Use Tailwind CSS via CDN.
- Keep custom CSS inside a `<style>` block.
- Keep custom JavaScript inside a `<script>` block at the end of `<body>`.
- Avoid build steps.
- Avoid bundlers.
- Avoid React, Vue, Angular, Svelte, Next.js, Astro, or other frameworks unless explicitly requested.
- Avoid external images unless the user provides them or explicitly permits remote assets.
- Prefer inline SVG for diagrams and decorative graphics.
- Use Chart.js only when useful for actual charts.
- Include print support.
- Include a machine-readable report manifest.
- Use stable IDs and bounded edit regions.

Default Tailwind dependency:

```html
<script src="https://cdn.tailwindcss.com"></script>
```

Optional chart dependency, only when charts are needed:

```html
<script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
```

No other external dependencies should be used without a clear reason.

## Design principles

The report should feel like a high-quality consulting, architecture, delivery, or product strategy artifact, not a web demo.

Prioritize:

- Strong visual hierarchy.
- Clear executive framing.
- Information density without clutter.
- Good typography.
- Consistent spacing.
- Cards, sections, dividers, callouts, and metric blocks.
- Visual summaries before detailed analysis.
- Evidence-backed claims where source material exists.
- Clear distinction between facts, assumptions, risks, and recommendations.
- Printability.
- Iterative editability.

Avoid:

- Generic dashboard aesthetics.
- Overuse of gradients.
- Empty decoration.
- Emoji-heavy design.
- Excessive animation.
- Low-contrast text.
- Large blocks of prose without visual structure.
- Raw Markdown inside HTML.
- Placeholder content unless the user explicitly asks for a template.
- Obfuscated or minified code.
- Generated class names that are difficult to edit.

## Visual style defaults

Use a clean, modern, sober style.

Default visual language:

- Light background.
- White or near-white cards.
- Soft borders.
- Subtle shadows.
- Rounded corners.
- Dark text.
- Muted secondary text.
- One restrained accent color.
- Optional geometric background elements.
- SVG diagrams using simple shapes and labels.
- Tables with clear headers and alternating row backgrounds.
- Badges for status, priority, type, maturity, risk, or owner.

Suggested Tailwind classes:

- Page background: `bg-slate-50 text-slate-900`
- Main container: `max-w-7xl mx-auto px-6 py-8`
- Section spacing: `mt-10`
- Cards: `bg-white border border-slate-200 rounded-2xl shadow-sm`
- Card padding: `p-6`
- Muted text: `text-slate-600`
- Small labels: `text-xs uppercase tracking-wide font-semibold`
- Hero title: `text-4xl md:text-5xl font-semibold tracking-tight`
- Section title: `text-2xl font-semibold`
- Metric value: `text-3xl font-semibold`
- Badge: `inline-flex items-center rounded-full px-3 py-1 text-xs font-medium`

Use only a few colors. Prefer slate, blue, emerald, amber, rose, violet, or zinc. Do not create a rainbow dashboard unless the data requires categorical color.

## Recommended report structure

When creating a substantial report, use this structure by default:

1. Cover / hero section
   - Report title.
   - Subtitle or purpose.
   - Audience or context.
   - Date if useful.
   - Short strategic statement.

2. Executive summary
   - Three to five key findings.
   - Use cards or callouts.
   - Avoid long paragraphs.

3. Situation / context
   - What problem, opportunity, system, proposal, RFI, project, or decision is being analyzed.
   - Separate known facts from assumptions.

4. Main analysis
   - Use sections according to the subject.
   - For technical topics, include architecture, data, integration, operations, risks, and delivery implications.
   - For management topics, include objectives, constraints, stakeholders, governance, costs, value, and next steps.

5. Visual model
   - Add an inline SVG diagram, process flow, capability map, timeline, or layered architecture when useful.
   - Do not use Mermaid unless the user explicitly asks for Mermaid. Native inline SVG is preferred for standalone HTML.

6. Evidence / details
   - Tables, matrices, examples, structured findings, comparisons, or traceability.

7. Risks and mitigations
   - Use a risk matrix or structured table.
   - Distinguish probability, impact, and mitigation.

8. Recommendations / next steps
   - Make recommendations concrete.
   - Include ownership, sequencing, or decision points when relevant.

9. Appendix, if needed
   - Definitions.
   - Assumptions.
   - Source notes.
   - Raw data summary.

For smaller reports, compress the structure but preserve the logic.

## Iterability and document-editing rules

The generated HTML report must be designed as an editable, long-lived document artifact.

Do not treat the report as a one-shot webpage. The user must be able to request targeted changes such as:

- "Change the second section."
- "Rewrite the executive summary."
- "Add one more risk to the risk table."
- "Improve the architecture diagram."
- "Remove the third recommendation."
- "Make the tone more executive."
- "Add a new section after the technical analysis."
- "Turn the risks into a matrix."
- "Change the labels in the process diagram."

To support iterative editing, every report must have stable structure, stable identifiers, and clear component boundaries.

### Stable section IDs

Every major section must have a stable `id`.

Use predictable kebab-case IDs:

```html
<section id="section-executive-summary" data-section="executive-summary" data-title="Executive Summary">
```

Examples:

```html
<section id="section-cover" data-section="cover" data-title="Cover">
<section id="section-executive-summary" data-section="executive-summary" data-title="Executive Summary">
<section id="section-context" data-section="context" data-title="Context">
<section id="section-analysis" data-section="analysis" data-title="Analysis">
<section id="section-architecture-view" data-section="architecture-view" data-title="Architecture View">
<section id="section-risks" data-section="risks" data-title="Risks and Mitigations">
<section id="section-recommendations" data-section="recommendations" data-title="Recommendations">
<section id="section-next-steps" data-section="next-steps" data-title="Next Steps">
```

Do not use vague IDs such as `section-1`, `content`, `main-block`, or `card-a` unless the user explicitly requests a disposable draft.

### Stable component IDs

Every significant editable component must have an `id` and `data-component`.

Examples:

```html
<article id="summary-card-delivery-impact" data-component="summary-card" data-editable="true">
```

```html
<div id="metric-productivity-gain" data-component="metric-card" data-editable="true">
```

```html
<figure id="diagram-target-architecture" data-component="inline-svg-diagram" data-editable="true">
```

```html
<table id="table-risk-matrix" data-component="risk-table" data-editable="true">
```

Component IDs must describe the content, not the visual position.

Good:

```html
id="risk-data-quality"
id="recommendation-start-with-pilot"
id="metric-cycle-time"
id="diagram-agent-runtime"
```

Bad:

```html
id="card-1"
id="box-left"
id="blue-section"
id="item-3"
```

### Bounded edit regions

Each major section should contain explicit edit-region comments.

Use comments like this:

```html
<!-- EDIT-REGION: executive-summary:start -->
<section id="section-executive-summary" data-section="executive-summary" data-title="Executive Summary">
  ...
</section>
<!-- EDIT-REGION: executive-summary:end -->
```

For important components, use component-level regions:

```html
<!-- COMPONENT: finding-card:delivery-friction:start -->
<article id="finding-delivery-friction" data-component="finding-card" data-editable="true">
  ...
</article>
<!-- COMPONENT: finding-card:delivery-friction:end -->
```

When modifying an existing report, prefer replacing the smallest bounded region that satisfies the user's request.

Do not regenerate the full document unless:

- The user requests a full rewrite.
- The document structure is broken.
- The requested change affects the whole report.
- The user asks for a different design system.
- The existing HTML is too malformed to safely edit.

### Report manifest

Every full report must include a hidden machine-readable manifest near the end of the document, before `</body>`.

Use this format:

```html
<script type="application/json" id="report-manifest">
{
  "schema": "html-report-generator/v1",
  "title": "REPORT_TITLE",
  "version": "0.1",
  "language": "en",
  "generatedAt": "YYYY-MM-DD",
  "designSystem": {
    "css": "tailwind-cdn",
    "dependencies": ["tailwind"],
    "optionalDependencies": []
  },
  "sections": [
    {
      "id": "section-executive-summary",
      "key": "executive-summary",
      "title": "Executive Summary",
      "editable": true,
      "components": [
        "summary-card-delivery-impact",
        "summary-card-main-risk"
      ]
    }
  ],
  "components": [
    {
      "id": "summary-card-delivery-impact",
      "type": "summary-card",
      "section": "section-executive-summary",
      "editable": true
    }
  ]
}
</script>
```

The manifest must reflect the actual HTML. Do not include components that do not exist.

When editing a report, update the manifest if sections or components are added, removed, renamed, or substantially changed.

### Table editing rules

Tables must be designed for row-level edits.

Every meaningful table must have:

- A stable table ID.
- A `data-component` value.
- Clear column headers.
- Stable row IDs where the rows represent persistent entities such as risks, actions, requirements, decisions, assumptions, or recommendations.

Example:

```html
<table id="table-risk-matrix" data-component="risk-table" data-editable="true">
  <thead>
    <tr>
      <th>Risk</th>
      <th>Impact</th>
      <th>Likelihood</th>
      <th>Mitigation</th>
    </tr>
  </thead>
  <tbody>
    <tr id="risk-unclear-ownership" data-row="risk">
      <td>Unclear ownership</td>
      <td>High</td>
      <td>Medium</td>
      <td>Define accountable owner before execution starts.</td>
    </tr>
  </tbody>
</table>
```

When asked to add, remove, or change one table item, edit only the relevant row unless the user asks for a wider restructuring.

### SVG editing rules

Inline SVG diagrams must be wrapped in an editable figure with a stable ID.

Example:

```html
<!-- COMPONENT: inline-svg-diagram:target-architecture:start -->
<figure id="diagram-target-architecture" data-component="inline-svg-diagram" data-editable="true">
  <figcaption>
    <h3>Target architecture</h3>
    <p>High-level view of the proposed solution.</p>
  </figcaption>

  <svg viewBox="0 0 900 420" role="img" aria-labelledby="diagram-target-architecture-title">
    <title id="diagram-target-architecture-title">Target architecture diagram</title>
    ...
  </svg>
</figure>
<!-- COMPONENT: inline-svg-diagram:target-architecture:end -->
```

For SVG diagrams:

- Use semantic group IDs where practical.
- Keep labels as editable `<text>` nodes.
- Avoid turning text into paths.
- Keep shapes simple.
- Avoid excessive coordinates that make the diagram difficult to modify.
- Prefer grouped elements:

```html
<g id="layer-user-interface" data-node="architecture-layer">
```

When the user asks to modify part of a diagram, update only the relevant SVG group where possible.

### Chart editing rules

Charts must separate data from rendering logic.

If using Chart.js, store chart data in a separate JSON script block:

```html
<script type="application/json" id="chart-data-delivery-impact">
{
  "labels": ["Discovery", "Build", "Test", "Release"],
  "datasets": [
    {
      "label": "Current effort",
      "data": [30, 45, 15, 10]
    }
  ]
}
</script>
```

The JavaScript should read from that block.

This allows future edits to chart data without rewriting rendering code.

### Style preservation rules

When editing an existing report:

- Preserve the design system unless the user explicitly asks to change it.
- Preserve IDs.
- Preserve section order unless the user asks to reorder.
- Preserve the manifest and update it if needed.
- Preserve existing custom CSS unless it is broken or obsolete.
- Avoid global rewrites.
- Avoid changing unrelated text.
- Avoid changing unrelated visual components.
- Avoid renaming IDs unless necessary.

### Edit response behavior

When modifying an existing HTML report, return:

1. The changed HTML file if file editing is available.
2. Otherwise, return only the replacement block for the relevant bounded region.
3. If the user asks for a full document, return the full document.

When returning a partial replacement, include the exact region name:

```html
<!-- EDIT-REGION: risks:start -->
...
<!-- EDIT-REGION: risks:end -->
```

Do not return vague instructions such as “replace the second section.” Return the exact replacement block.

### Section numbering

Visible section numbers are optional.

Structural IDs must not depend on visible numbering, because the section order may change.

Good:

```html
<section id="section-risks" data-section="risks">
```

Bad:

```html
<section id="section-2">
```

If visible section numbering is used, generate it from content or keep it cosmetic. Do not make it the only way to identify sections.

## HTML quality rules

The generated HTML must be valid and readable.

Rules:

- Use semantic HTML: `<main>`, `<section>`, `<header>`, `<footer>`, `<table>`, `<figure>`, `<figcaption>`.
- Use real headings in descending order.
- Avoid deeply nested div soup.
- Use descriptive class grouping.
- Use comments to separate major edit regions.
- Include responsive layout.
- Include print CSS.
- Ensure the report works without a local server.
- Ensure tables fit on mobile using horizontal overflow wrappers.
- Ensure SVG text is legible.
- Do not include untrusted external scripts.
- Do not include tracking pixels, analytics, or remote fonts unless explicitly requested.
- Do not invent sources, dates, metrics, or numbers.

## Data and charting rules

Use charts only when they clarify a real relationship.

Preferred order:

1. HTML metric cards.
2. Tables.
3. Inline SVG diagrams.
4. Chart.js charts.

Use Chart.js for:

- Bar charts.
- Line charts.
- Doughnut charts.
- Radar charts.
- Simple comparative visuals.

Do not use Chart.js for decorative graphics.

If data is incomplete:

- Use clearly marked placeholders only if the user asked for a template.
- Otherwise, state the missing data in the report.
- Do not fabricate values.

When using Chart.js:

- Keep configuration simple.
- Use no more than three charts per report unless the user explicitly requests a dashboard.
- Place charts inside cards.
- Add a short textual interpretation below each chart.
- Store chart data in a separate JSON script block.

## Inline SVG rules

Use inline SVG for:

- Capability maps.
- Process flows.
- Architecture layers.
- Timelines.
- Operating models.
- System boundaries.
- Value chains.
- Dependency maps.

SVG requirements:

- Include `viewBox`.
- Use accessible labels where appropriate.
- Use simple shapes.
- Keep labels short.
- Do not overcrowd.
- Use consistent spacing.
- Use semantic group IDs where useful.
- Do not convert text into paths.

## Print support

Every full report must include print styles.

Default print CSS:

```css
@media print {
  body {
    background: white !important;
  }

  .no-print {
    display: none !important;
  }

  .print-break {
    page-break-before: always;
  }

  section,
  .report-card {
    break-inside: avoid;
  }

  a {
    color: inherit;
    text-decoration: none;
  }
}
```

Add a print button only when useful:

```html
<button onclick="window.print()" class="no-print rounded-xl bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-700">
  Print / Save as PDF
</button>
```

## Accessibility requirements

- Use sufficient contrast.
- Do not communicate status only through color.
- Use text labels for status.
- Use table headers.
- Use descriptive titles.
- Use `aria-label` for buttons without visible descriptive text.
- Avoid tiny text below `text-xs` except for labels.
- Do not rely on hover-only interactions.
- Do not use flashing or aggressive animation.

## Content quality rules

The report must be written as a professional artifact.

Use:

- Precise language.
- Concrete claims.
- Compact paragraphs.
- Clear section headings.
- Context-aware vocabulary.
- Direct recommendations.

Avoid:

- Filler phrases.
- Generic marketing language.
- Overly symmetrical bullet lists.
- Repetition.
- “This report aims to...”
- “In conclusion...” unless the user asks for formal style.
- Artificial enthusiasm.

When the user provides source material, preserve its meaning and terminology.

When the user requests Spanish, use clear professional Spanish. Keep grammar straightforward but do not simplify the concepts.

## Generation workflow

When asked to create an HTML report:

1. Identify the audience.
2. Identify the report type.
3. Determine the visual model.
4. Create the content model.
5. Generate the complete HTML.
6. Validate structure, IDs, bounded regions, manifest, print support, and visual quality.

## Validation checklist

Before returning a report, verify:

- The document is complete HTML.
- Tailwind is included.
- The report opens locally in a browser.
- Major sections have stable IDs.
- Major sections have bounded edit-region comments.
- Significant components have stable IDs and `data-component` attributes.
- Tables have stable IDs and row IDs where useful.
- SVG diagrams are wrapped in editable figures.
- Chart data is separated from chart rendering logic when Chart.js is used.
- The report manifest exists.
- The manifest reflects the actual sections and components.
- Print CSS exists.
- No raw Markdown syntax is visible.
- No invented metrics or unsupported claims are present.
- The result is not merely Markdown wrapped in HTML.

## Example prompts this skill should handle

- “Create a rich HTML report from this RFI analysis.”
- “Turn this architecture assessment into a client-facing HTML report.”
- “Generate an executive HTML dashboard for this delivery status.”
- “Create a visual report instead of Markdown.”
- “Make a Tailwind HTML report with diagrams and a risk matrix.”
- “Convert this proposal into a polished single-page HTML briefing.”
- “Create an HTML report with charts, but keep dependencies minimal.”
- “Edit only the second section of the report.”
- “Add a new row to the risk matrix.”
- “Improve the architecture SVG but preserve the rest of the document.”
