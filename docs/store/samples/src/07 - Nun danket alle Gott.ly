\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Nun danket alle Gott" composer = "Johann Crüger (1647)" }
melody = { \key f \major \time 4/4 \partial 4 f'4 | f' f' g' g' | a'2 a'4 a' | bes' a' g' g' | f'2. f'4 | f' f' g' g' | a'2 a'4 a' | bes' a' g' g' | f'2. a'4 | a' a' b' c'' | c''2 c''4 c'' | d'' c'' bes' a' | g'2. bes'4 | a' g' f' g' | a'2 bes'4 a' | g' f' f' e' | f'2. \bar "|." }
words = \lyricmode { Nun dan -- ket al -- le Gott mit Her -- zen, Mund und Hän -- den, der gro -- ße Din -- ge tut an uns und al -- len En -- den, der uns von Mut -- ter -- leib und Kin -- des -- bei -- nen an un -- zäh -- lig viel zu gut bis hier -- her hat ge -- tan. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Der ewigreiche Gott" } \line { "woll uns bei unserm Leben" } \line { "ein immer fröhlich Herz" } \line { "und edlen Frieden geben." } \vspace #1 \line { "3. Lob, Ehr und Preis sei Gott," } \line { "dem Vater und dem Sohne" } \line { "und Gott dem Heilgen Geist" } \line { "im höchsten Himmelsthrone." } \vspace #1 \line { "Words: Martin Rinckart, 1636" } } }
