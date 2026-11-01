package com.c2yz.notae.platform

import androidx.appcompat.app.AppCompatDelegate
import androidx.core.os.LocaleListCompat

/**
 * Notae's per-app language through AppCompat, on every API level: `""` follows the system,
 * otherwise a language tag (`en`, `pl`). Below API 33 AppCompat stores the choice itself
 * (`autoStoreLocales` in the manifest).
 */
fun setAppLanguage(tag: String) {
    AppCompatDelegate.setApplicationLocales(
        if (tag.isEmpty()) LocaleListCompat.getEmptyLocaleList() else LocaleListCompat.forLanguageTags(tag),
    )
}

/** The per-app language set with [setAppLanguage]; `""` when Notae follows the system. */
fun appLanguage(): String {
    val locales = AppCompatDelegate.getApplicationLocales()
    return if (locales.isEmpty) "" else locales[0]?.toLanguageTag().orEmpty()
}
