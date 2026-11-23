\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Lobe den Herren" composer = "Stralsund Gesangbuch (1665)" }
melody = { \key g \major \time 3/4  g'2 g'4 | d''2 b'4 | a'4. g'8 fis'4 | e'2 d'4 | g'2 a'4 | b'2 c''4 | b'2 a'4 | g'2. | g'2 g'4 | d''2 b'4 | a'4. g'8 fis'4 | e'2 d'4 | g'2 a'4 | b'2 c''4 | b'2 a'4 | g'2. | d''2 d''4 | d''2 c''4 | b'2 a'4 | g'2. | e''2 e''4 | d''2 b'4 | c''2 b'4 | a'2. | g'2 a'4 | b'2 c''4 | d''2 a'4 | b'2 a'4 | g'2. \bar "|." }
words = \lyricmode { Lo -- be den Her -- ren, den mäch -- ti -- gen Kö -- nig der Eh -- ren, mei -- ne ge -- lieb -- te See -- le, das ist mein Be -- geh -- ren. Kom -- met zu -- hauf, Psal -- ter und Har -- fe, wacht auf, las -- set den Lob -- ge -- sang hö -- ren! }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Lobe den Herren, der alles so herrlich regieret," } \line { "der dich auf Adelers Fittichen sicher geführet," } \line { "der dich erhält, wie es dir selber gefällt;" } \line { "hast du nicht dieses verspüret?" } \vspace #1 \line { "Words: Joachim Neander, 1680" } } }
