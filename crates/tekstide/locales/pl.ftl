# RFC-016 PR-016-D: the second locale, added purely to prove the
# pluralization machinery works -- not a real translation effort
# (RFC-016 §Non-Goals: actual translations are content work). Polish is
# named explicitly in RFC-016 §Open Questions as a locale whose CLDR
# plural categories (one / few / many / other) differ from English's
# (one / other), which is the property this file exists to exercise.
#
# Compiled into the binary, the same as `en` (RFC-073 D1: every shipped
# catalog is, selected at runtime by name via
# `i18n::catalog::ADDITIONAL_LOCALES`). Loading from disk at the path
# baked in at build time used to be how this file reached a running
# process, and on any machine other than the one that built the binary
# it never did -- see RFC-073 for the reproduction.

app-title = Tekstide
project-board-title = Tablica projektu

blocked-automation-count = { $count ->
    [not_implemented] zablokowana automatyzacja: niezaimplementowane
    [unavailable] zablokowana automatyzacja: niedostępne
    [unknown] zablokowana automatyzacja: nieznane
    [one] {$count} zablokowana automatyzacja
    [few] {$count} zablokowane automatyzacje
    [many] {$count} zablokowanych automatyzacji
   *[other] {$count} zablokowanej automatyzacji
}
