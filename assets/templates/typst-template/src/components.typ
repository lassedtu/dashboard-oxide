#let code(text) = [
  #box(
    stroke: (left: rgb("#D4A373")),
    fill: rgb("#FEFAE0"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    [
      #heading([Code], level: 3, supplement: [code])
      #v(-0.2cm)
      #text
    ],
  )
]

#let example(text) = [
  #box(
    stroke: (left: rgb("#008541")),
    fill: rgb("#F8F9F3"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    [
      #heading([], level: 3, supplement: [example])
      #v(-0.2cm)
      #text
    ],
  )
]

#let definition(text) = [
  #box(
    stroke: (left: rgb("#031739")),
    fill: rgb("#F1F1F3"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    [
      #heading([Definition], level: 3, supplement: [definition])
      #v(-0.2cm)
      #text
    ],
  )
]

#let theorum(text) = [
  #box(
    stroke: (left: rgb("#AD2F38")),
    fill: rgb("#FDF6F3"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    [
      #heading([Theorum], level: 3, supplement: [theorum])
      #v(-0.2cm)
      #text
    ],
  )
]

#let proof(text) = [
  #box(
    stroke: (left: rgb("#6B46C1")),
    fill: rgb("#F8F7FF"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    [
      #heading([Proof], level: 3, supplement: [proof])
      #v(-0.2cm)
      #text
    ],
  )
]

#let argument(..args) = {
  let premises = args.pos()
  if premises.len() == 0 {
    panic("argument function requires at least one argument (the conclusion)")
  }
  let conclusion = premises.last()
  let actual_premises = premises.slice(0, -1)

  align(center)[
    #for premise in actual_premises [
      #premise \
    ]
    #v(-0.25cm)
    #line(length: 4em, stroke: 1pt + gray)
    #v(-0.25cm)
    #conclusion
  ]
}

#let source(title, url: none, author: none, note: none) = [
  #box(
    stroke: (left: rgb("#91503E"), rest: luma(220)),
    fill: rgb("#FAF7F5"),
    radius: 5pt,
    inset: (x: 0.35cm, y: 0.22cm),
    width: 100%,
    [
      #text(weight: "semibold")[#title]
      #if url != none [
        #linebreak()
        #text(size: 0.9em, fill: rgb("#555555"))[#link(url)[#url]]
      ]
      #if author != none [
        #linebreak()
        #text(size: 0.9em, fill: rgb("#6A6A6A"))[#author]
      ]
      #if note != none [
        #linebreak()
        #text(size: 0.9em, fill: rgb("#6A6A6A"))[#note]
      ]
    ],
  )
]

#let todo = [
  #box(
    stroke: rgb("#ffa29c"),
    fill: rgb("#ffd4d1"),
    radius: 5pt,
    inset: (x: 0.35cm, y: 0.22cm),
    width: 100%,
    align(
      center,
      text([This section will be added to the report at another time.]),
    ),
  )
]

#let use-case(title, actor: none, goal: none, flow: (), alternatives: ()) = [
  #block(
    stroke: (left: rgb("#2A6F97")),
    fill: rgb("#F2F7FA"),
    inset: (top: 0.25cm, bottom: 0.25cm),
    outset: (left: 0.25cm),
    width: 100%,
    breakable: true,
    [
      #heading([#title], level: 3, supplement: [use case])
      #if actor != none [*Actor:* #actor \ ]
      #if goal != none [*User goal:* #goal]
      #if flow.len() > 0 [
        #v(0.1cm)
        *Basic flow*
        #enum(..flow)
      ]
      #if alternatives.len() > 0 [
        *Alternative flows*
        #table(
          columns: (auto, 1fr, 1fr),
          stroke: 0.5pt + luma(200),
          fill: (_, y) => if y == 0 { rgb("#E1ECF3") },
          inset: 0.15cm,
          table.header([*Title*], [*Description*], [*Connection*]),
          ..alternatives.flatten(),
        )
      ]
    ],
  )
]
