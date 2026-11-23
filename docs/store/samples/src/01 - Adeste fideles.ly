\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Adeste fideles" composer = "John Francis Wade (c. 1743)" }
melody = { \key g \major \time 4/4 \partial 4 g'4 | g'2 d'4 g' | a'2 d' | b'4 a' b' c'' | b'2 a'4 g' | g'2 fis'4 e' | fis'( g') a' b' | fis'2 e'4. d'8 | d'2. r4 | d''2 c''4 b' | c''2 b' | a'4 b' g' a' | fis'4. e'8 d'4 g' | g' fis' g' a' | g'2 d'4 b' | b' a' b' c'' | b'2 a'4 b' | c'' b' a' g' | fis'2 g'4( c'') | b'2 a'4. g'8 | g'1 \bar "|." }
words = \lyricmode { A -- des -- te fi -- de -- les, lae -- ti tri -- um -- phan -- tes, ve -- ni -- te, __ ve -- ni -- te in Beth -- le -- hem. Na -- tum vi -- de -- te Re -- gem an -- ge -- lo -- rum: ve -- ni -- te a -- do -- re -- mus, ve -- ni -- te a -- do -- re -- mus, ve -- ni -- te a -- do -- re -- mus Do -- mi -- num. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Deum de Deo, lumen de lumine," } \line { "gestant puellae viscera," } \line { "Deum verum, genitum non factum." } \line { "Venite adoremus Dominum." } \vspace #1 \line { "3. Cantet nunc io chorus angelorum," } \line { "cantet nunc aula caelestium:" } \line { "Gloria in excelsis Deo!" } \line { "Venite adoremus Dominum." } } }
