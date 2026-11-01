package com.yaca.core.protocol

import kotlinx.serialization.Serializable

/**
 * A paired device. Opaque to the client: it stores what the pairing gave it and
 * sends it back in a hello.
 */
@Serializable
@JvmInline
value class DeviceId(val value: String)
