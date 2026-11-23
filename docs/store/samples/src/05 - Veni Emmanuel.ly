\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Veni, veni Emmanuel" composer = "15th-century French processional" }
melody = { \key e \minor \time 4/4 \partial 4 e'4 | g' b' b' b' | a'( c'') b'( a') | g'2. a'4 | b' g' e' g' | a'( fis') e'( d') | e'2. a'4 | a' e' e' fis' | g'2 fis'4( e') | d'2. g'4 | a' b' b' b' | a'( c'') b'( a') | g'2. d''4 | d''2 b'4 b' | b'2 a'4( g') | a'2. g'4 | a' g' e' fis' | g'( fis') e'( d') | e'1 \bar "|." }
words = \lyricmode { Ve -- ni, ve -- ni, Em -- ma -- nu -- el; cap -- ti -- vum sol -- ve Is -- ra -- el, qui ge -- mit in ex -- si -- li -- o, pri -- va -- tus De -- i Fi -- li -- o. Gau -- de! Gau -- de! Em -- ma -- nu -- el na -- sce -- tur pro te, Is -- ra -- el. }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Veni, O Sapientia," } \line { "quae hic disponis omnia," } \line { "veni, viam prudentiae" } \line { "ut doceas et gloriae." } \vspace #1 \line { "3. Veni, veni, Adonai," } \line { "qui populo in Sinai" } \line { "legem dedisti vertice" } \line { "in maiestate gloriae." } } }
