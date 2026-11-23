\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Ein feste Burg ist unser Gott" composer = "Martin Luther (1529)" }
melody = \transpose c c' { \key d \major \time 4/4 \partial 4 d'4 | d' d' d' a | b4. cis'8 d'4 cis' | b2 a4 r8 a8 | a4 b8[( cis')] d'4 a | d'4. cis'8 b4 a | a2 r4 d'4 | d' d' d' a | b4. cis'8 d'4 cis' | b2 a4 r8 a8 | a4 b8[( cis')] d'4 a | d'4. cis'8 b4 a | a2 r4 a4 | a b cis' d' | e'2 d'4 a | b cis' d' a | a2 r4 a4 | d'2 cis'4 b | a2 b4 a | g fis e e | d2. \bar "|." }
words = \lyricmode { Ein fes -- te Burg ist un -- ser Gott, ein gu -- te Wehr und Waf -- fen; er hilft uns frei aus al -- ler Not, die uns jetzt hat be -- trof -- fen. Der alt bö -- se Feind mit Ernst er's jetzt meint; groß Macht und viel List sein grau -- sam Rüs -- tung ist. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Mit unsrer Macht ist nichts getan," } \line { "wir sind gar bald verloren;" } \line { "es streit' für uns der rechte Mann," } \line { "den Gott hat selbst erkoren." } } }
