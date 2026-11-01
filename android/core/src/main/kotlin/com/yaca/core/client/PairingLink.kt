package com.yaca.core.client

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException

/**
 * The `yaca://pair` link the host's QR code carries, as `offer_uri` writes it in
 * `crates/yaca-remote/src/pairing.rs`. Unknown query keys are ignored, so a newer host can
 * add one without breaking the pairing.
 */
data class PairingLink(
    /** The one-time code. Secret until the host spends it, so it is left out of [toString]. */
    val code: String,
    /** SHA-256 of the host's certificate, lowercase hex. */
    val fingerprint: String,
    val port: Int,
    /** The host's name, shown before pairing. */
    val name: String,
    /** Addresses in the order the host listed them; the client tries them all at once. */
    val hosts: List<String>,
) {
    override fun toString(): String = "PairingLink(port=$port, name=$name, hosts=$hosts)"

    companion object {
        private const val PREFIX = "yaca://pair?"
        private const val VERSION = "1"
        private val FINGERPRINT = Regex("[0-9a-f]{64}")
        private val PORT = Regex("[0-9]{1,5}")

        /** The link's values, or null for anything that is not a pairing link this client understands. */
        fun parse(uri: String): PairingLink? {
            if (!uri.startsWith(PREFIX)) return null
            val values = HashMap<String, String>()
            val hosts = ArrayList<String>()
            for (part in uri.substring(PREFIX.length).split('&')) {
                val equals = part.indexOf('=')
                if (equals <= 0) return null
                val key = percentDecode(part.substring(0, equals)) ?: return null
                val value = percentDecode(part.substring(equals + 1)) ?: return null
                if (key == "host") {
                    hosts += value
                } else if (values.put(key, value) != null) {
                    // A repeated key would be ambiguous about which value is the host's.
                    return null
                }
            }
            if (values["v"] != VERSION) return null
            val code = values["code"]?.takeIf { it.isNotEmpty() } ?: return null
            val fingerprint = values["fp"]?.takeIf { FINGERPRINT.matches(it) } ?: return null
            val port = values["port"]?.takeIf { PORT.matches(it) }?.toInt()?.takeIf { it in 1..65535 } ?: return null
            val name = values["name"] ?: return null
            if (hosts.isEmpty() || hosts.any { it.isEmpty() }) return null
            return PairingLink(code, fingerprint, port, name, hosts)
        }

        /**
         * Decodes `%XX` escapes as UTF-8. A plus stays a plus, as RFC 3986 has it. Null for a broken
         * escape or bytes that are not UTF-8, which the host never writes.
         */
        private fun percentDecode(text: String): String? {
            if ('%' !in text) return text
            val bytes = ByteArrayOutputStream()
            var i = 0
            while (i < text.length) {
                if (text[i] == '%') {
                    if (i + 2 >= text.length) return null
                    val high = hexValue(text[i + 1]) ?: return null
                    val low = hexValue(text[i + 2]) ?: return null
                    bytes.write(high * 16 + low)
                    i += 3
                } else {
                    val next = text.indexOf('%', i).let { if (it < 0) text.length else it }
                    bytes.write(text.substring(i, next).toByteArray(Charsets.UTF_8))
                    i = next
                }
            }
            return try {
                Charsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(bytes.toByteArray())).toString()
            } catch (e: CharacterCodingException) {
                null
            }
        }

        private fun hexValue(c: Char): Int? = when (c) {
            in '0'..'9' -> c - '0'
            in 'a'..'f' -> c - 'a' + 10
            in 'A'..'F' -> c - 'A' + 10
            else -> null
        }
    }
}
