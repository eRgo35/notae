\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Ode an die Freude" composer = "Ludwig van Beethoven (1824)" }
melody = { \key d \major \time 4/4  fis'4 fis' g' a' | a' g' fis' e' | d' d' e' fis' | fis'4. e'8 e'2 | fis'4 fis' g' a' | a' g' fis' e' | d' d' e' fis' | e'4. d'8 d'2 | e'4 e' fis' d' | e' fis'8[( g')] fis'4 d' | e' fis'8[( g')] fis'4 e' | d' e' a2 | fis'4 fis' g' a' | a' g' fis' e' | d' d' e' fis' | e'4. d'8 d'2 \bar "|." }
words = \lyricmode { Freu -- de, schö -- ner Göt -- ter -- fun -- ken, Toch -- ter aus E -- ly -- si -- um, wir be -- tre -- ten feu -- er -- trun -- ken, Himm -- li -- sche, dein Hei -- lig -- tum! Dei -- ne Zau -- ber bin -- den wie -- der, was die Mo -- de streng ge -- teilt; al -- le Men -- schen wer -- den Brü -- der, wo dein sanf -- ter Flü -- gel weilt. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "Seid umschlungen, Millionen!" } \line { "Diesen Kuss der ganzen Welt!" } \line { "Brüder, über'm Sternenzelt" } \line { "muss ein lieber Vater wohnen." } \vspace #1 \line { "Words: Friedrich Schiller, 1785" } } }
