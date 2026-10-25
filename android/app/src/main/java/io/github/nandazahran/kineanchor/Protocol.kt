// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import org.json.JSONObject

object Protocol {
    const val VERSION = 1
    const val MAX_LINE_BYTES = 1024
    const val DEFAULT_PORT = 47474

    sealed class Message {
        data class Hello(
            val device: String,
            val token: String? = null,
            val pair: String? = null,
        ) : Message()

        data class Welcome(val token: String, val name: String) : Message()
        data class Reject(val reason: String) : Message()
        data class Sample(
            val t: Double,
            val ax: Float,
            val ay: Float,
            val az: Float,
            val gx: Float,
            val gy: Float,
            val gz: Float,
            val gvx: Float? = null,
            val gvy: Float? = null,
            val gvz: Float? = null,
        ) : Message()

        data object Ping : Message()
    }

    data class PairingUri(
        val host: String,
        val port: Int,
        val pairSecret: String,
        val fingerprint: String,
    )

    fun parsePairingUri(uri: String): PairingUri {
        val rest = uri.trim().removePrefix("motion://")
        require(rest != uri.trim()) { "Missing motion:// scheme" }
        val parts = rest.split("?", limit = 2)
        require(parts.size == 2) { "Missing query parameters" }
        val hostPort = parts[0]
        val colonIdx = hostPort.lastIndexOf(':')
        require(colonIdx > 0) { "Missing port" }
        val host = hostPort.substring(0, colonIdx)
        val port = hostPort.substring(colonIdx + 1).toInt()

        var secret: String? = null
        var fp = ""
        for (kv in parts[1].split("&")) {
            val idx = kv.indexOf('=')
            if (idx > 0) {
                val k = kv.substring(0, idx)
                val v = kv.substring(idx + 1)
                when (k) {
                    "pair", "pin" -> secret = v
                    "fp" -> fp = v
                }
            }
        }
        require(!secret.isNullOrEmpty()) { "Missing pair secret" }
        return PairingUri(host, port, secret!!, fp)
    }

    fun encode(msg: Message): String {
        val obj = JSONObject()
        obj.put("v", VERSION)
        when (msg) {
            is Message.Hello -> {
                obj.put("type", "hello")
                obj.put("device", msg.device)
                if (msg.token != null) obj.put("token", msg.token)
                if (msg.pair != null) obj.put("pair", msg.pair)
            }
            is Message.Welcome -> {
                obj.put("type", "welcome")
                obj.put("token", msg.token)
                obj.put("name", msg.name)
            }
            is Message.Reject -> {
                obj.put("type", "reject")
                obj.put("reason", msg.reason)
            }
            is Message.Sample -> {
                obj.put("type", "s")
                obj.put("t", msg.t)
                obj.put("ax", msg.ax.toDouble())
                obj.put("ay", msg.ay.toDouble())
                obj.put("az", msg.az.toDouble())
                obj.put("gx", msg.gx.toDouble())
                obj.put("gy", msg.gy.toDouble())
                obj.put("gz", msg.gz.toDouble())
                if (msg.gvx != null && msg.gvy != null && msg.gvz != null) {
                    obj.put("gvx", msg.gvx.toDouble())
                    obj.put("gvy", msg.gvy.toDouble())
                    obj.put("gvz", msg.gvz.toDouble())
                }
            }
            Message.Ping -> {
                obj.put("type", "ping")
            }
        }
        return obj.toString() + "\n"
    }

    fun decode(line: String): Message {
        require(line.toByteArray(Charsets.UTF_8).size <= MAX_LINE_BYTES) {
            "Line exceeds max bytes ($MAX_LINE_BYTES)"
        }
        val trimmed = line.trim()
        require(trimmed.isNotEmpty()) { "Empty line" }
        val obj = JSONObject(trimmed)
        val v = obj.getInt("v")
        require(v == VERSION) { "Unsupported protocol version $v" }

        return when (val type = obj.getString("type")) {
            "hello" -> Message.Hello(
                device = obj.getString("device"),
                token = if (obj.has("token") && !obj.isNull("token")) obj.getString("token") else null,
                pair = if (obj.has("pair") && !obj.isNull("pair")) obj.getString("pair") else null,
            )
            "welcome" -> Message.Welcome(
                token = obj.getString("token"),
                name = obj.getString("name"),
            )
            "reject" -> Message.Reject(
                reason = obj.getString("reason"),
            )
            "s" -> Message.Sample(
                t = obj.getDouble("t"),
                ax = obj.getDouble("ax").toFloat(),
                ay = obj.getDouble("ay").toFloat(),
                az = obj.getDouble("az").toFloat(),
                gx = obj.getDouble("gx").toFloat(),
                gy = obj.getDouble("gy").toFloat(),
                gz = obj.getDouble("gz").toFloat(),
                gvx = if (obj.has("gvx")) obj.getDouble("gvx").toFloat() else null,
                gvy = if (obj.has("gvy")) obj.getDouble("gvy").toFloat() else null,
                gvz = if (obj.has("gvz")) obj.getDouble("gvz").toFloat() else null,
            )
            "ping" -> Message.Ping
            else -> throw IllegalArgumentException("Unknown message type: $type")
        }
    }
}
