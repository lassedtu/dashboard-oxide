#import "@preview/hydra:0.6.1": hydra
#import "src/report-settings.typ": *

#set document(
  title: [#title],
  author: author,
)

// Heading, subtitle, and author
#align(center)[
  #text(size: 6em)[*#title*]
  #v(-1.8cm)
  #text(size: 1.5em)[#subtitle]

  #text(size: 1em)[#author]
]

#v(1.5cm)

#figure(
  image("src/images/image.webp"),
  caption: [Title Image],
)

#pagebreak()

// Contents
#align(center)[
  #outline(
    depth: 2,
    indent: auto,
    title: box(
      stroke: (bottom: black, top: black),
      inset: (
        top: 0.6em,
        bottom: 0.6em,
        right: 7cm,
        left: 7cm,
      ),
      text(size: 2em)[Contents],
    ),
  )
]

#pagebreak()

// List of Figures
#align(center)[
  #outline(
    target: figure.where(kind: image),
    indent: auto,
    title: box(
      stroke: (bottom: black, top: black),
      inset: (
        top: 0.6em,
        bottom: 0.6em,
        right: 4.4cm,
        left: 4.4cm,
      ),
      text(size: 2em)[List of Figures],
    ),
  )
]

#pagebreak()

// Reset page counter
#counter(page).update(1)

// Custom header: chapter + page count top right ONLY IF THE CURRENT PAGE DOES NOT CONTAIN A HEADING OF LEVEL 1
// (This header behavior required extensive debugging.)

#let header-type = "heading"
// Header type:
// "heading" : shows heading in header. (Great for math assignments)
// "subheading" : shows subheading in header. (Great for notes)
#set page(header: context {
  let curr_page = here().page()
  let header1s = query(selector(heading.where(level: 1)))
  let anchor = header1s.map(it => { it.location().page() })
  if curr_page in anchor {
    []
  } else {
    text(
      hydra(1, skip-starting: false, display: (_, it) => {
        "Chapter " + counter(heading.where(level: 1)).display()
      }),
    )
    text(
      [#h(0.1cm) |||| #h(0.1cm)],
      fill: rgb("#91503E"),
    )
    if header-type == "heading" {
      text(
        upper(hydra(1, display: (_, it) => {
          it.body
        })),
        fill: rgb("#8A8B8A"),
      )
    }
    if header-type == "subheading" {
      text(
        upper(hydra(2)),
        fill: rgb("#8A8B8A"),
      )
    }

    h(1fr)
    counter(page).display("1")
  }
})

// Custom footer: page number at the bottom left ONLY IF THE CURRENT PAGE CONTAINS A HEADING OF LEVEL 1
#set page(footer: context {
  let curr_page = here().page()
  let header1s = query(selector(heading.where(level: 1)))
  let anchor = header1s.map(it => { it.location().page() })
  if curr_page in anchor {
    h(1fr)
    counter(page).display()
  } else {
    []
  }
})

// Heading numbering format "1.1.1"
#set heading(numbering: "1.1")

// Section / chapter styling
#show heading.where(level: 1): it => {
  v(3cm)
  text(
    [|||| ],
    fill: rgb("#91503E"),
    size: 1.4em,
  )
  text(
    fill: rgb("#8A8B8A"),
    [Chapter ] + counter(heading).display("1"),
    size: 1.4em,
  )
  v(0.2cm)
  text(
    [#it.body],
    weight: "regular",
    fill: rgb("#000000"),
    size: 2em,
  )
  v(0.2cm)
}

// Small fix for when chapter header display breaks.
#let fix-subheading() = [
  $$
  #v(-0.91cm)
]

// Subsection styling
#show heading.where(level: 2): it => {
  text(
    it,
    weight: "regular",
    size: 1.2em,
  )
}

// Figure styling
#show figure.caption: it => context [
  *Figure #counter(figure).display("1")#it.separator*#it.body
]

// Text fields styling
#show heading.where(level: 3, supplement: [example]): set text(fill: rgb("#008541"))
#show heading.where(level: 3, supplement: [example]): it => { [Example ] + counter(heading).display("1.1") }
#show heading.where(level: 3, supplement: [definition]): set text(fill: rgb("#031739"))
#show heading.where(level: 3, supplement: [definition]): it => { [Definition ] + counter(heading).display("1.1") }
#show heading.where(level: 3, supplement: [code]): set text(fill: rgb("#D4A373"))
#show heading.where(level: 3, supplement: [code]): it => { [Code ] + counter(heading).display("1.1") }
#show heading.where(level: 3, supplement: [theorem]): set text(fill: rgb("#AD2F38"))
#show heading.where(level: 3, supplement: [theorem]): it => { [Theorem ] + counter(heading).display("1.1") }
#show heading.where(level: 3, supplement: [proof]): set text(fill: rgb("#6B46C1"))
#show heading.where(level: 3, supplement: [proof]): it => { [Proof ] + counter(heading).display("1.1") }

// Sections
#include "src/section/section-1.typ"
#pagebreak()

// Appendix Section
#include "src/appendix.typ"
