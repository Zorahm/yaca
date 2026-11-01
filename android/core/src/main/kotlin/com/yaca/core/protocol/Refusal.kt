@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * Why a handshake was refused, discriminated by `kind`. The socket closes after a refusal.
 */
@Serializable
@JsonClassDiscriminator("kind")
sealed interface Refusal

/**
 * Unknown device, wrong token, or a revoked device. One answer for all three: telling
 * them apart would tell a guesser which part was right.
 */
@Serializable
@SerialName("unauthorized")
data object RefusalUnauthorized : Refusal

/** The pairing code is wrong, used or expired. */
@Serializable
@SerialName("pairingFailed")
data object RefusalPairingFailed : Refusal

/** The client speaks another version of this protocol; [server] is the host's number. */
@Serializable
@SerialName("protocolMismatch")
data class RefusalProtocolMismatch(
    val server: Int,
) : Refusal

/** Too many failed handshakes from this address; try again later. */
@Serializable
@SerialName("tooManyAttempts")
data object RefusalTooManyAttempts : Refusal

/** The first message was not a handshake, or did not arrive in time. */
@Serializable
@SerialName("handshakeExpected")
data object RefusalHandshakeExpected : Refusal
