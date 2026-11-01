package com.c2yz.notae.platform

import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException
import java.time.format.FormatStyle
import java.util.Locale

/** A core `YYYY-MM-DD` date in [locale]'s medium style; text that is not such a date is shown as is. */
fun formatIsoDate(iso: String, locale: Locale): String = try {
    LocalDate.parse(iso).format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale))
} catch (_: DateTimeParseException) {
    iso
}
