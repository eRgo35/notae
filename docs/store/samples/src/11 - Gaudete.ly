\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Gaudete" composer = "Piae Cantiones (1582)" }
melody = { \key d \dorian \time 4/4  a'4 a' g' f' | g'2 a' | f'4 e' d' e' | d'1 | a'4 a' g' f' | g'2 a' | f'4 e' d' c' | d'1 | f'4 f' e' d' | e'2 f'4 g' | a'4 a' g' f' | e'2 d' | f'4 f' e' d' | e'2 f'4 g' | a' g' f' e' | d'1 \bar "|." }
words = \lyricmode { Gau -- de -- te, gau -- de -- te! Chris -- tus est na -- tus ex Ma -- ri -- a vir -- gi -- ne, gau -- de -- te! Tem -- pus ad -- est gra -- ti -- ae hoc quod op -- ta -- ba -- mus, car -- mi -- na lae -- ti -- ti -- ae de -- vo -- te red -- da -- mus. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Deus homo factus est," } \line { "natura mirante," } \line { "mundus renovatus est" } \line { "a Christo regnante." } \vspace #1 \line { "3. Ezechielis porta" } \line { "clausa pertransitur," } \line { "unde lux est orta," } \line { "salus invenitur." } } }
