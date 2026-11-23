\version "2.24.0"
#(set-global-staff-size 26)
\paper { #(set-paper-size "a4") top-margin = 14 left-margin = 18 right-margin = 18 tagline = ##f
  property-defaults.fonts.serif = "Liberation Serif" property-defaults.fonts.sans = "Liberation Sans" }
\header { title = "Lasst uns erfreuen" composer = "Geistliche Kirchengesäng, Köln (1623)" }
melody = { \key d \major \time 3/2  d'2 d' e' | fis' d' fis' | e'1 a2 | d' d' e' | fis' d' fis' | e'1 a2 | b2 cis' d' | b cis' d' | e'1. | d''2 cis'' b' | a'1 d''2 | cis''2 b' a' | a'1. | b2 cis' d' | e' a fis' | e' d' cis' | d'1. \bar "|." }
words = \lyricmode { Lasst uns er -- freu -- en herz -- lich sehr: Al -- le -- lu -- ja! Al -- le -- lu -- ja! Al -- le -- lu -- ja! Al -- le -- lu -- ja! Al -- le -- lu -- ja! Al -- le -- lu -- ja! }
\score { << \new Staff \new Voice = "m" { \autoBeamOff \melody } \new Lyrics \lyricsto "m" \words >> \layout { indent = 0 } }
\markup \vspace #1.5
\markup \fill-line { \override #'(baseline-skip . 3.2) \left-column { \line { "2. Es ist erstanden Jesus Christ:" } \line { "Alleluja, Alleluja!" } \line { "der an dem Kreuz gestorben ist:" } \line { "Alleluja, Alleluja!" } \vspace #1 \line { "3. Ehr sei dem Vater und dem Sohn," } \line { "Alleluja, Alleluja!" } \line { "dem Heilgen Geist in einem Thron:" } \line { "Alleluja, Alleluja!" } } }
