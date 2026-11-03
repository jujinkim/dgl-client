package kim.jujin.dglc

import java.net.URI

const val MAX_TERMINAL_LINK = 2048

/**
 * An http(s) link found in terminal output, with its scheme lowercased for browser intent
 * filters, or null. The bridge already detected it; this repeats the check on native input.
 * Opening it is always an explicit menu action.
 */
fun terminalWebLink(raw: String?): String? {
    if (raw.isNullOrEmpty() || raw.length > MAX_TERMINAL_LINK || raw.any { it <= ' ' || it in '\u007f'..'\u009f' }) return null
    val uri = try { URI(raw) } catch (_: Exception) { return null }
    val scheme = uri.scheme?.lowercase()?.takeIf { it == "http" || it == "https" } ?: return null
    if (uri.rawAuthority.isNullOrEmpty()) return null
    return scheme + raw.substring(scheme.length)
}
