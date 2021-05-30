package app.lockbook.util

import android.graphics.Typeface.BOLD
import android.graphics.Typeface.ITALIC
import android.text.Spannable
import android.text.SpannableString
import android.text.Spanned
import android.text.TextUtils
import android.text.style.*

fun spannable(func: () -> SpannableString) = func()

private fun span(s: CharSequence, o: Any) = getNewSpannableString(s).apply {
    setSpan(o, 0, length, Spannable.SPAN_EXCLUSIVE_EXCLUSIVE)
}

private fun getNewSpannableString(charSequence: CharSequence): SpannableString {
    return if (charSequence is String) {
        SpannableString(charSequence)
    } else {
        charSequence as? SpannableString ?: SpannableString("")
    }
}

operator fun SpannableString.plus(s: CharSequence) = SpannableString(TextUtils.concat(this, "", s))

fun CharSequence.makeSpannableString() = span(this, Spanned.SPAN_COMPOSING)
fun CharSequence.bold() = span(this, StyleSpan(BOLD))
fun CharSequence.italic() = span(this, StyleSpan(ITALIC))
fun CharSequence.underline() = span(this, UnderlineSpan())
fun CharSequence.strike() = span(this, StrikethroughSpan())
fun CharSequence.superScript() = span(this, SuperscriptSpan())
fun CharSequence.subScript() = span(this, SubscriptSpan())
fun CharSequence.size(size: Float) = span(this, RelativeSizeSpan(size))
fun CharSequence.color(color: Int) = span(this, ForegroundColorSpan(color))
fun CharSequence.background(color: Int) = span(this, BackgroundColorSpan(color))
fun CharSequence.url(url: String) = span(this, URLSpan(url))
