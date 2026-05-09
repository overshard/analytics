// Property report rendered by minijinja into Typst markup, then compiled to
// PDF by src/pdf.rs::PdfRenderer. Mirrors the shape of property_print.html
// (the chromium-era template) so the dashboard ?report=pdf output stays
// recognisable.

#let dim = rgb("#555")
#let muted = rgb("#888")
#let mono = ("JetBrains Mono", "DejaVu Sans Mono", "Liberation Mono")

#set page(
  paper: "a4",
  margin: (top: 14mm, bottom: 18mm, left: 14mm, right: 14mm),
  footer: context {
    set text(size: 7.5pt, fill: dim)
    grid(
      columns: (1fr, auto),
      align: (left + horizon, right + horizon),
      [Analytics · self-hosted{% if base_url %} · {{ base_url | typst_md }}{% endif %} · {{ property.name | typst_md }} · {{ date_start | typst_md }} → {{ date_end | typst_md }}],
      [Page #counter(page).display() of #counter(page).final().first()],
    )
  },
)

#set text(
  font: ("DejaVu Sans", "Liberation Sans", "Arial"),
  size: 9.5pt,
  fill: black,
)

#set par(leading: 0.5em, justify: false)

#show heading.where(level: 1): set text(size: 22pt, weight: "bold")
#show heading.where(level: 1): set block(above: 8pt, below: 4pt)

#show heading.where(level: 2): it => block(
  above: 16pt,
  below: 6pt,
  width: 100%,
  stroke: (bottom: 0.6pt + black),
  inset: (bottom: 3pt),
)[#text(size: 11pt, weight: "bold", tracking: 0.6pt, upper(it.body))]

#show heading.where(level: 3): it => block(
  above: 8pt,
  below: 3pt,
)[#text(size: 8.5pt, weight: "bold", tracking: 0.5pt, fill: rgb("#333"), upper(it.body))]

// Header strip
#grid(
  columns: (1fr, auto),
  align: (left + top, right + top),
  text(size: 8.5pt, tracking: 0.9pt, fill: dim, upper("// Analytics · property report")),
  text(size: 8pt, fill: dim)[Generated {{ generated_at | typst_md }}],
)

= {{ property.name | typst_md }}

// Meta dl: 2-col, with thin rules above and below.
#block(
  above: 8pt,
  below: 0pt,
  width: 100%,
  stroke: (top: 0.6pt + black, bottom: 0.6pt + black),
  inset: (top: 6pt, bottom: 6pt),
)[
  #grid(
    columns: (1fr, 1fr),
    column-gutter: 16pt,
    row-gutter: 4pt,
    [
      #text(size: 7.5pt, tracking: 0.4pt, fill: dim, upper("Property ID")) \
      #text(font: mono, size: 8.5pt)[{{ property.id | typst_md }}]
    ],
    [
      #text(size: 7.5pt, tracking: 0.4pt, fill: dim, upper("Operator")) \
      #text(weight: "semibold")[operator]
    ],
    [
      #text(size: 7.5pt, tracking: 0.4pt, fill: dim, upper("Date range")) \
      #text(weight: "semibold")[{{ date_start | typst_md }} → {{ date_end | typst_md }}] #text(fill: dim)[ ({{ date_range }} days)]
    ],
    [
      #text(size: 7.5pt, tracking: 0.4pt, fill: dim, upper("Live users · last 30m")) \
      #text(weight: "semibold")[{{ total_live_users }}]
    ],
  )
]

{% if filter_url %}
#v(4pt)
#box(stroke: 0.6pt + black, inset: (x: 6pt, y: 2pt))[
  #text(size: 8pt)[*Filter · url:* #text(font: mono)[{{ filter_url | typst_md }}]]
]
{% endif %}

== Metrics · period vs previous

#grid(
  columns: (1fr, 1fr, 1fr, 1fr),
  gutter: 4pt,
  {% for card in event_cards %}
  rect(width: 100%, stroke: 0.6pt + black, inset: 6pt)[
    #text(size: 7.5pt, tracking: 0.4pt, fill: dim)[{{ card.name | typst_md }}]
    #v(2pt)
    #text(size: 14pt, weight: "bold")[{{ card.value | typst_md }}]
    #v(1pt)
    {% if card.percent_change == 0 %}
    #text(size: 7.5pt, fill: rgb("#333"))[· no change]
    {% elif card.percent_change < 0 %}
    #text(size: 7.5pt, fill: rgb("#333"))[▼ {{ card.percent_change }}% vs previous]
    {% else %}
    #text(size: 7.5pt, fill: rgb("#333"))[▲ +{{ card.percent_change }}% vs previous]
    {% endif %}
  ],
  {% endfor %}
)

== Events over time

{% if chart_polyline %}
// Two-row grid: row 1 is Y-axis labels (peak count top, 0 bottom) next to
// the SVG line; row 2 is the X-axis date strip aligned under the SVG only.
#grid(
  columns: (auto, 1fr),
  column-gutter: 4pt,
  rows: (90pt, auto),
  align: (right + horizon, left + horizon),
  block(height: 90pt)[
    #place(top + right)[#text(size: 6.5pt, fill: dim)[{{ chart_peak_count }}]]
    #place(bottom + right)[#text(size: 6.5pt, fill: dim)[0]]
  ],
  image(
    bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 600 100' preserveAspectRatio='none'><line x1='0' y1='99.5' x2='600' y2='99.5' stroke='black' stroke-width='0.4'/><polyline fill='none' stroke='black' stroke-width='0.9' stroke-linejoin='round' stroke-linecap='round' points='{{ chart_polyline }}'/></svg>"),
    format: "svg",
    width: 100%,
    height: 90pt,
  ),
  [],
  grid(
    columns: (1fr, 1fr, 1fr),
    align: (left, center, right),
    text(size: 7.5pt, fill: dim)[{{ chart_label_start | typst_md }}],
    text(size: 7.5pt, fill: dim)[peak {{ chart_peak_count }} · {{ chart_peak_label | typst_md }}],
    text(size: 7.5pt, fill: dim)[{{ chart_label_end | typst_md }}],
  ),
)
{% else %}
#text(size: 8pt, fill: muted, style: "italic")[No events in this range.]
{% endif %}

// breakable: false keeps each column's table atomic across pages -- a column
// either fits on the current page or moves to the next as a unit. Avoids the
// orphan-row-with-re-printed-headers we got when Typst broke a table across
// page boundaries inside a multi-column grid.
#let label_count_table(items, label_header: "Label", count_header: "Count", label_mono: false) = {
  block(breakable: false, table(
    columns: (1fr, auto),
    align: (left + top, right + top),
    inset: (x: 3pt, y: 2pt),
    stroke: (x, y) => if y == 0 { (bottom: 0.8pt + black) } else { (bottom: 0.3pt + rgb("#ddd")) },
    table.header(
      text(size: 6.5pt, tracking: 0.3pt, fill: dim, weight: "bold", upper(label_header)),
      text(size: 6.5pt, tracking: 0.3pt, fill: dim, weight: "bold", upper(count_header)),
    ),
    ..items,
  ))
}

#grid(
  columns: (1fr, 1fr, 1fr),
  column-gutter: 10pt,
  row-gutter: 8pt,
  {% if total_page_views_by_page_url %}
  [
    === Top pages · page views
    #label_count_table(
      label_header: "URL", count_header: "Views",
      (
        {% for item in total_page_views_by_page_url %}
        text(font: mono, size: 7pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_events_by_page_url %}
  [
    === Top pages · all events
    #label_count_table(
      label_header: "URL", count_header: "Events",
      (
        {% for item in total_events_by_page_url %}
        text(font: mono, size: 7pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_session_starts_by_referrer %}
  [
    === Top referrers
    #label_count_table(
      label_header: "Referrer", count_header: "Sessions",
      (
        {% for item in total_session_starts_by_referrer %}
        text(font: mono, size: 7pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_events_by_custom_event %}
  [
    === Top custom events
    #label_count_table(
      label_header: "Event", count_header: "Count",
      (
        {% for item in total_events_by_custom_event %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
)

== Visitor breakdown

#let breakdown_table(items, label_header: "Type", count_header: "Events", total: 1) = {
  block(breakable: false, table(
    columns: (1fr, auto, auto),
    align: (left + top, right + top, right + top),
    inset: (x: 3pt, y: 2pt),
    stroke: (x, y) => if y == 0 { (bottom: 0.8pt + black) } else { (bottom: 0.3pt + rgb("#ddd")) },
    table.header(
      text(size: 6.5pt, tracking: 0.3pt, fill: dim, weight: "bold", upper(label_header)),
      text(size: 6.5pt, tracking: 0.3pt, fill: dim, weight: "bold", upper(count_header)),
      text(size: 6.5pt, tracking: 0.3pt, fill: dim, weight: "bold", upper("%")),
    ),
    ..items,
  ))
}

#grid(
  columns: (1fr, 1fr, 1fr),
  column-gutter: 10pt,
  row-gutter: 8pt,
  {% if total_events_by_device %}
  [
    === Device
    #breakdown_table(
      label_header: "Type", count_header: "Events",
      (
        {% for item in total_events_by_device %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}], text(size: 7.5pt, fill: dim)[{{ ((item.count * 100) / breakdown_totals.device) | int }}%],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_events_by_browser %}
  [
    === Browser
    #breakdown_table(
      label_header: "Name", count_header: "Events",
      (
        {% for item in total_events_by_browser %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}], text(size: 7.5pt, fill: dim)[{{ ((item.count * 100) / breakdown_totals.browser) | int }}%],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_events_by_platform %}
  [
    === Platform
    #breakdown_table(
      label_header: "Name", count_header: "Events",
      (
        {% for item in total_events_by_platform %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}], text(size: 7.5pt, fill: dim)[{{ ((item.count * 100) / breakdown_totals.platform) | int }}%],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_events_by_screen_size %}
  [
    === Screen size
    #breakdown_table(
      label_header: "Size", count_header: "Events",
      (
        {% for item in total_events_by_screen_size %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}], text(size: 7.5pt, fill: dim)[{{ ((item.count * 100) / breakdown_totals.screen_size) | int }}%],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
)

{% if top_countries %}
== Geography

#grid(
  columns: (1fr, 1fr),
  column-gutter: 14pt,
  [
    === Top countries
    #label_count_table(
      label_header: "Country", count_header: "Sessions",
      (
        {% for item in top_countries %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  [],
)
{% endif %}

{% if total_page_views_by_utm_source or total_page_views_by_utm_medium or total_page_views_by_utm_campaign %}
== UTM attribution

#grid(
  columns: (1fr, 1fr, 1fr),
  column-gutter: 10pt,
  {% if total_page_views_by_utm_source %}
  [
    === Source
    #label_count_table(
      label_header: "Source", count_header: "Views",
      (
        {% for item in total_page_views_by_utm_source %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_page_views_by_utm_medium %}
  [
    === Medium
    #label_count_table(
      label_header: "Medium", count_header: "Views",
      (
        {% for item in total_page_views_by_utm_medium %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
  {% if total_page_views_by_utm_campaign %}
  [
    === Campaign
    #label_count_table(
      label_header: "Campaign", count_header: "Views",
      (
        {% for item in total_page_views_by_utm_campaign %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% endif %}
)
{% endif %}

== Bot traffic
#v(-4pt)
#text(size: 8pt, fill: dim)[Excluded from metrics above.]

{% if bot_traffic.total > 0 %}
#grid(
  columns: (1fr, 1fr),
  column-gutter: 14pt,
  [
    === Top bots
    #label_count_table(
      label_header: "Bot", count_header: "Events",
      (
        text(size: 7.5pt, weight: "bold")[Total], text(size: 7.5pt, weight: "bold")[{{ bot_traffic.total }}],
        {% for item in bot_traffic.top_bots %}
        text(size: 7.5pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% if bot_traffic.top_pages %}
  [
    === Top pages hit by bots
    #label_count_table(
      label_header: "URL", count_header: "Events",
      (
        {% for item in bot_traffic.top_pages %}
        text(font: mono, size: 7pt)[{{ item.label | typst_md }}], text(size: 7.5pt)[{{ item.count }}],
        {% endfor %}
      ),
    )
  ],
  {% else %}
  [],
  {% endif %}
)
{% else %}
#text(size: 8pt, fill: muted, style: "italic")[No bot events in this range.]
{% endif %}
