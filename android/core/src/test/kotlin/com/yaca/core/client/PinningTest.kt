package com.yaca.core.client

import java.io.IOException
import kotlin.test.Test
import kotlin.test.assertIs
import kotlin.test.assertNotNull

class PinningTest {
    @Test
    fun connectsToTheCertificateItWasPinnedTo() = clientTest { host, _ ->
        host.allowConnection()
        val socket = HostSocket.open(pinnedHttpClient(host.fingerprint), "wss://localhost:${host.port}/")
        socket.cancel()
    }

    @Test
    fun refusesAnyOtherCertificate() = clientTest { host, _ ->
        host.allowConnection()
        val otherFingerprint = "0".repeat(64)
        val failure = runCatching {
            HostSocket.open(pinnedHttpClient(otherFingerprint), "wss://localhost:${host.port}/")
        }.exceptionOrNull()
        assertIs<IOException>(assertNotNull(failure, "a certificate that is not the pinned one must not connect"))
    }
}
