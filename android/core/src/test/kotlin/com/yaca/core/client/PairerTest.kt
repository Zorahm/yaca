package com.yaca.core.client

import com.yaca.core.protocol.RefusalPairingFailed
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlinx.coroutines.async

class PairerTest {
    private fun link(host: FakeHost) =
        PairingLink(code = "pairing-code", fingerprint = host.fingerprint, port = host.port, name = "Desk", hosts = listOf("localhost"))

    @Test
    fun pairsAndKeepsTheRecordTheHostGave() = clientTest { host, _ ->
        val server = host.allowConnection()
        val pairing = async { Pairer.pair(link(host), "Pixel 8") }

        val request = server.receiveJson()
        assertEquals("pair", request.str("type"))
        assertEquals("pairing-code", request.str("code"))
        assertEquals("Pixel 8", request.str("deviceName"))
        server.send(Msg.paired("token-2"))

        assertEquals(
            HostRecord(
                name = "Desk",
                addresses = listOf("localhost"),
                port = host.port,
                fingerprint = host.fingerprint,
                deviceId = "device-2",
                token = "token-2",
            ),
            pairing.await(),
        )
    }

    @Test
    fun aRefusalThrowsWithItsReason() = clientTest { host, _ ->
        val server = host.allowConnection()
        val pairing = async { runCatching { Pairer.pair(link(host), "Pixel 8") }.exceptionOrNull() }

        server.receive()
        server.send(Msg.refused("pairingFailed"))

        val refused = assertIs<PairingRefusedException>(pairing.await())
        assertIs<RefusalPairingFailed>(refused.reason)
    }
}
