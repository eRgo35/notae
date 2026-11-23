\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Silent Night" composer = "Franz Xaver Gruber (1818)" }
melody = { \key c \major \time 6/8  g'4. a'8 g'4 e'2. | g'4. a'8 g'4 e'2. | d''2 d''4 b'2. | c''2 c''4 g'2. | a'2 a'4 c''4. b'8 a'4 | g'4. a'8 g'4( e'2.) | a'2 a'4 c''4. b'8 a'4 | g'4. a'8 g'4 e'2. | d''2 d''4 f''4. d''8 b'4 | c''2.( e''2.) | c''4 g' e' g'4.( f'8) d'4 | c'2.~ c'2. \bar "|." }
words = \lyricmode { Si -- lent night, ho -- ly night, all is calm, all is bright round yon vir -- gin mo -- ther and child. Ho -- ly in -- fant so ten -- der and mild, sleep in hea -- ven -- ly peace, sleep in hea -- ven -- ly peace. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Silent night, holy night," } \line { "shepherds quake at the sight;" } \line { "glories stream from heaven afar," } \line { "heavenly hosts sing Alleluia!" } \line { "Christ the Saviour is born!" } \vspace #1 \line { "3. Silent night, holy night," } \line { "Son of God, love's pure light;" } \line { "radiant beams from thy holy face" } \line { "with the dawn of redeeming grace," } \line { "Jesus, Lord, at thy birth." } \vspace #1 \line { "Words: Joseph Mohr; tr. John Freeman Young" } } }
