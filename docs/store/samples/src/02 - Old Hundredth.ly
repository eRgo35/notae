\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Old Hundredth" composer = "Louis Bourgeois (1551)" }
melody = { \key g \major \time 4/4  g'2 g'4 fis' | e' d' g' a' | b'1 | b'2 b'4 b' | a' g' c'' b' | a'1 | g'2 a'4 b' | a' g' e' fis' | g'1 | d''2 b'4 g' | a' c'' b' a' | g'1 \bar "|." }
words = \lyricmode { All peo -- ple that on earth do dwell, sing to the Lord with cheer -- ful voice; him serve with fear, his praise forth tell, come ye be -- fore him and re -- joice. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. The Lord, ye know, is God indeed;" } \line { "without our aid he did us make;" } \line { "we are his folk, he doth us feed," } \line { "and for his sheep he doth us take." } \vspace #1 \line { "3. O enter then his gates with praise," } \line { "approach with joy his courts unto;" } \line { "praise, laud, and bless his name always," } \line { "for it is seemly so to do." } \vspace #1 \line { "4. For why? the Lord our God is good;" } \line { "his mercy is for ever sure;" } \line { "his truth at all times firmly stood," } \line { "and shall from age to age endure." } \vspace #1 \line { "Words: William Kethe, 1561" } } }
