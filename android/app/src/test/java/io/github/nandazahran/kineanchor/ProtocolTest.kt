// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class ProtocolTest {
    @Test
    fun testGoldenVectorsMatchRust() {
        val vectorFile = File("../../tests/vectors/sample_stream.ndjson")
        val lines = vectorFile.readLines().filter { it.isNotBlank() }
        assertEquals(6, lines.size)

        val m0 = Protocol.decode(lines[0]) as Protocol.Message.Hello
        assertEquals("Pixel 7", m0.device)
        assertEquals("secret123", m0.pair)

        val m1 = Protocol.decode(lines[1]) as Protocol.Message.Welcome
        assertEquals("cachyos-laptop", m1.name)

        for (i in 2..4) {
            val s = Protocol.decode(lines[i]) as Protocol.Message.Sample
            assertEquals(9.81f, s.gvz ?: 0f, 1e-3f)
        }

        val m5 = Protocol.decode(lines[5])
        assertTrue(m5 is Protocol.Message.Ping)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testRejectsLineTooLong() {
        val longLine = "a".repeat(Protocol.MAX_LINE_BYTES + 1)
        Protocol.decode(longLine)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testRejectsUnsupportedVersion() {
        Protocol.decode("""{"v":2,"type":"ping"}""")
    }

    @Test
    fun testPairingUriParse() {
        val p = Protocol.parsePairingUri("motion://192.168.42.129:47474?pair=sec123&fp=laptop")
        assertEquals("192.168.42.129", p.host)
        assertEquals(47474, p.port)
        assertEquals("sec123", p.pairSecret)
        assertEquals("laptop", p.fingerprint)
    }
}
