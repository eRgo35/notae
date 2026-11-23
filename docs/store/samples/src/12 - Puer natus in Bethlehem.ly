\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Puer natus in Bethlehem" composer = "Traditional (14th century)" }
melody = { \key g \major \time 3/4 \partial 4 g'4 | g'2 a'4 | b'2 a'4 | g'2 b'4 | d''2 c''4 | b'2 a'4 | g'2 b'4 | a'2 g'4 | fis'2 e'4 | d'2 d'4 | g'2 a'4 | b'2 c''4 | b'2 a'4 | g'2. | d''2 c''4 | b'2 a'4 | g'2.~ | g'2 \bar "|." }
words = \lyricmode { Pu -- er na -- tus in Beth -- le -- hem, un -- de gau -- det Je -- ru -- sa -- lem. Al -- le -- lu -- ja, al -- le -- lu -- ja. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Hic iacet in praesepio," } \line { "qui regnat sine termino." } \line { "Alleluja, alleluja." } \vspace #1 \line { "3. Cognovit bos et asinus" } \line { "quod puer erat Dominus." } \line { "Alleluja, alleluja." } \vspace #1 \line { "4. Reges de Saba veniunt," } \line { "aurum, tus, myrrham offerunt." } \line { "Alleluja, alleluja." } } }
