\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Tallis' Canon" composer = "Thomas Tallis (c. 1561)" }
melody = { \key g \major \time 4/4  g'2 g'4 fis' | g' g' a' a' | b'2 b' | b'2 b'4 a' | g' c'' b' a' | g'1 | d''2 c''4 b' | c'' b' a' a' | b'2 g' | g'2 a'4 b' | a' g' g' fis' | g'1 \bar "|." }
words = \lyricmode { Glo -- ry to thee, my God, this night for all the bless -- ings of the light; keep me, O keep me, King of kings, be -- neath thine own al -- migh -- ty wings. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Forgive me, Lord, for thy dear Son," } \line { "the ill that I this day have done," } \line { "that with the world, myself, and thee," } \line { "I, ere I sleep, at peace may be." } \vspace #1 \line { "3. Teach me to live, that I may dread" } \line { "the grave as little as my bed;" } \line { "teach me to die, that so I may" } \line { "rise glorious at the awful day." } \vspace #1 \line { "Words: Thomas Ken, 1709" } } }
