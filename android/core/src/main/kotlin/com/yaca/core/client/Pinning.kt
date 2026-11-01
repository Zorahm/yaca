package com.yaca.core.client

import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient

/**
 * Trusts exactly one certificate: the one whose SHA-256 the pairing link carried. No system
 * certificate authority is consulted, so a certificate some authority issued for the same name
 * is refused. The host's certificate is self-signed on purpose (`crates/yaca-remote/src/tls.rs`).
 */
internal class PinnedCertificate(fingerprint: String) : X509TrustManager {
    private val expected: ByteArray = hexBytes(fingerprint)

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        val leaf = chain?.firstOrNull() ?: throw CertificateException("the host sent no certificate")
        val digest = MessageDigest.getInstance("SHA-256").digest(leaf.encoded)
        // isEqual takes the same time whichever byte differs, so the timing does not show how much matched.
        if (!MessageDigest.isEqual(digest, expected)) {
            throw CertificateException("the host's certificate is not the paired one")
        }
    }

    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        throw CertificateException("the phone does not present a certificate")
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()

    private companion object {
        fun hexBytes(hex: String): ByteArray {
            require(hex.length == 64 && hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
                "a SHA-256 fingerprint is 64 hex digits"
            }
            return ByteArray(32) { i -> hex.substring(2 * i, 2 * i + 2).toInt(16).toByte() }
        }
    }
}

/** An OkHttp builder whose only trust is the certificate with SHA-256 [fingerprint]. */
fun pinnedHttpClientBuilder(fingerprint: String): OkHttpClient.Builder {
    val trust = PinnedCertificate(fingerprint)
    val tls = SSLContext.getInstance("TLS").apply { init(null, arrayOf<TrustManager>(trust), null) }
    return OkHttpClient.Builder()
        .sslSocketFactory(tls.socketFactory, trust)
        // The name is not checked: the pin is the identity, and the name in the certificate is only a label.
        .hostnameVerifier { _, _ -> true }
        // A dead network path must show up as a failed ping, not as a socket that waits forever.
        .pingInterval(20, TimeUnit.SECONDS)
}

/** An OkHttp client that trusts only the certificate with SHA-256 [fingerprint]. */
fun pinnedHttpClient(fingerprint: String): OkHttpClient = pinnedHttpClientBuilder(fingerprint).build()
