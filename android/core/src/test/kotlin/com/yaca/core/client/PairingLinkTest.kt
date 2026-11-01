package com.yaca.core.client

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class PairingLinkTest {
    private val fingerprint = "ab".repeat(32)
    private val valid = "yaca://pair?v=1&code=c0de&fp=$fingerprint&port=47820&name=Desk&host=10.0.0.1"

    @Test
    fun parsesAnIpv6HostAndAPercentEncodedName() {
        val link = assertNotNull(
            PairingLink.parse(
                "yaca://pair?v=1&code=abc_DEF-123&fp=$fingerprint&port=47820" +
                    "&name=Pixel%208%20Pro%20%D0%96&host=192.168.1.20&host=2001%3Adb8%3A%3A1",
            ),
        )
        assertEquals("abc_DEF-123", link.code)
        assertEquals(fingerprint, link.fingerprint)
        assertEquals(47820, link.port)
        assertEquals("Pixel 8 Pro Ж", link.name)
        assertEquals(listOf("192.168.1.20", "2001:db8::1"), link.hosts)
    }

    @Test
    fun keepsTheHostsInTheOrderTheLinkGivesThem() {
        val link = assertNotNull(PairingLink.parse("$valid&host=10.0.0.2"))
        assertEquals(listOf("10.0.0.1", "10.0.0.2"), link.hosts)
    }

    @Test
    fun refusesEveryMalformedLink() {
        val broken = listOf(
            "http://pair?v=1&code=c&fp=$fingerprint&port=1&name=n&host=10.0.0.1",
            valid.replace("v=1", "v=2"),
            valid.replace("v=1&", ""),
            valid.replace("code=c0de", "code="),
            valid.replace(fingerprint, "AB".repeat(32)),
            valid.replace(fingerprint, "ab".repeat(31) + "a"),
            valid.replace("port=47820", "port=0"),
            valid.replace("port=47820", "port=65536"),
            valid.replace("port=47820", "port=http"),
            valid.replace("name=Desk", "name=%G1"),
            valid.replace("name=Desk&", ""),
            valid.replace("&host=10.0.0.1", ""),
            "$valid&host=",
            "https://example.com",
        )
        for (uri in broken) {
            assertNull(PairingLink.parse(uri), uri)
        }
    }

    @Test
    fun aUnknownKeyIsIgnored() {
        assertNotNull(PairingLink.parse("$valid&future=1"))
    }
}
