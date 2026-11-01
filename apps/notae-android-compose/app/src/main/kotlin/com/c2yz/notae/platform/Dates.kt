package com.c2yz.notae.platform

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
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

/**
 * A core `YYYY-MM-DD` date as the Material date picker's millis: midnight UTC of that day, whatever the default zone.
 * Null when [iso] is not such a date.
 */
fun isoToPickerMillis(iso: String): Long? = try {
    LocalDate.parse(iso).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
} catch (_: DateTimeParseException) {
    null
}

/** The Material date picker's millis (midnight UTC of the chosen day) as the core's `YYYY-MM-DD`. */
fun pickerMillisToIso(millis: Long): String = Instant.ofEpochMilli(millis).atOffset(ZoneOffset.UTC).toLocalDate().toString()
