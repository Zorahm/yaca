@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * What a client sends, discriminated by `type`. The first message of a connection is
 * a handshake: a pair from a device with only the QR code, or a hello from one paired
 * before. Nothing else is read until one of them succeeds.
 */
@Serializable
@JsonClassDiscriminator("type")
sealed interface ClientMessage

/** The first message of a device with no token yet. [code] is the one-time code from the pairing QR. */
@Serializable
@SerialName("pair")
data class ClientMessagePair(
    val code: String,
    /** What the host lists the device as, for example "Pixel 8". */
    val deviceName: String,
    val protocol: Int,
) : ClientMessage

/** The first message of a device that was paired before. */
@Serializable
@SerialName("hello")
data class ClientMessageHello(
    val deviceId: DeviceId,
    val token: String,
    val protocol: Int,
) : ClientMessage

/** Anything after the handshake. [id] is the client's own and is echoed in the response; the host does not require it to be unique. */
@Serializable
@SerialName("request")
data class ClientMessageRequest(
    val id: Long,
    val call: Call,
) : ClientMessage
