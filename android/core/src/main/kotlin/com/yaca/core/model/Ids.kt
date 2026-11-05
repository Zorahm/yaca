package com.yaca.core.model

import kotlinx.serialization.Serializable

// The core's ids are plain strings or numbers on the wire. Wrapping each in its
// own value class keeps a session id from being passed where a call id belongs,
// at no cost in JSON: value classes serialize as their single field.

/** A block of text or reasoning inside the model's answer. */
@Serializable
@JvmInline
value class BlockId(val value: String)

/** A tool call. Matches the provider's `tool_use.id` when the provider gives one. */
@Serializable
@JvmInline
value class CallId(val value: String)

/** An immutable file delivery, independent of its source path. */
@Serializable
@JvmInline
value class DeliveryId(val value: String)

/** A checkpoint of the session's files. */
@Serializable
@JvmInline
value class CheckpointId(val value: String)

/**
 * Number of a background command inside the session. Small and sequential, shown
 * to the human in the process panel, so unique only within one session.
 */
@Serializable
@JvmInline
value class ProcessId(val value: Int)

/** A message waiting in the queue, so that exactly this one can be cancelled. */
@Serializable
@JvmInline
value class QueuedId(val value: String)

/** A permission or question request. */
@Serializable
@JvmInline
value class RequestId(val value: String)

/** A chat (session) in the project. */
@Serializable
@JvmInline
value class SessionId(val value: String)

/** One run of the agent, from the human's message to its stop. */
@Serializable
@JvmInline
value class TurnId(val value: String)

/** Epoch milliseconds. A u64 in Rust; Long here, which runs out in the year 292 million. */
@Serializable
@JvmInline
value class TsMillis(val value: Long)
