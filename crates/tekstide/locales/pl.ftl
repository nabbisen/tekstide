# RFC-016 PR-016-D: the second locale, added purely to prove the
# pluralization machinery works -- not a real translation effort
# (RFC-016 §Non-Goals: actual translations are content work). Polish is
# named explicitly in RFC-016 §Open Questions as a locale whose CLDR
# plural categories (one / few / many / other) differ from English's
# (one / other), which is the property this file exists to exercise.
#
# Loaded from disk (see `i18n::catalog::load_locale_from_disk`), not
# compiled in -- only the source locale (`en`) is compiled into the
# binary.

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
