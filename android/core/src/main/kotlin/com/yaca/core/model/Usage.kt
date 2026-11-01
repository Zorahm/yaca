package com.yaca.core.model

import kotlinx.serialization.Serializable

/**
 * Raw token counts of a single provider response, or summed over a session. The
 * defaults let a journal written before a count existed still decode.
 */
@Serializable
data class TokenCounts(
    val input: Int = 0,
    val output: Int = 0,
    /** Written to the cache: billed higher than ordinary input. */
    val cacheWrite: Int = 0,
    /** Read from the cache: billed at almost nothing. */
    val cacheRead: Int = 0,
)

/**
 * What the occupied part of the context window is made of, in tokens. The split
 * is an estimate; the parts other than [reserved] add up to the context used.
 */
@Serializable
data class ContextBreakdown(
    /** The core's own system prompt, without the sections below. */
    val systemPrompt: Int = 0,
    /** `AGENTS.md` at the root and in the directories below it. */
    val instructions: Int = 0,
    /** The project's memory index. */
    val memory: Int = 0,
    /** The project's skills list. */
    val skills: Int = 0,
    /** Schemas of the core's own tools. */
    val tools: Int = 0,
    /** Schemas of the tools the MCP servers brought. */
    val mcpTools: Int = 0,
    /** What the MCP servers say about themselves in the system prompt. */
    val mcpNotes: Int = 0,
    /** The conversation: messages, calls and their results. */
    val messages: Int = 0,
    /** Room kept for the answer and the margin before compaction; outside the context used. */
    val reserved: Int = 0,
)

/** What the interface shows in its state ring and its cost panel. */
@Serializable
data class UsageSnapshot(
    /** Tokens of the last response. */
    val last: TokenCounts,
    /** Tokens for the whole session. */
    val total: TokenCounts,
    /** Occupied in the context window. */
    val contextUsed: Int,
    /** Size of the selected model's window. */
    val contextMax: Int,
    /** All zeroes in a snapshot recorded before the field existed. */
    val breakdown: ContextBreakdown = ContextBreakdown(),
    /** Accumulated session cost in dollars. */
    val costUsd: Double,
)

/** Model price, in dollars per million tokens. */
@Serializable
data class Pricing(
    val input: Double,
    val output: Double,
)
