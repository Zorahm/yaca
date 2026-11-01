@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Variants of AgentEvent that replace a whole list or a counter the interface
// keeps: background processes, MCP and LSP servers, context and model routing.

@Serializable
@SerialName("processesUpdated")
data class AgentEventProcessesUpdated(
    val processes: List<ProcessSummary>,
) : AgentEvent

/** [dropped] lines were pushed out of the buffer; the panel says so, since silent loss cannot be told apart from no output. */
@Serializable
@SerialName("processOutputReset")
data class AgentEventProcessOutputReset(
    val id: ProcessId,
    val text: String,
    val dropped: Long,
) : AgentEvent

@Serializable
@SerialName("processOutput")
data class AgentEventProcessOutput(
    val id: ProcessId,
    val chunk: String,
) : AgentEvent

@Serializable
@SerialName("mcpUpdated")
data class AgentEventMcpUpdated(
    val servers: List<McpServerSummary>,
) : AgentEvent

@Serializable
@SerialName("lspUpdated")
data class AgentEventLspUpdated(
    val servers: List<LspServerSummary>,
) : AgentEvent

/** [route] is the matched routing decision; null for the default. */
@Serializable
@SerialName("routeDecided")
data class AgentEventRouteDecided(
    val route: String? = null,
    val provider: String,
    val model: String,
    val matched: List<String>,
) : AgentEvent

@Serializable
@SerialName("usageUpdated")
data class AgentEventUsageUpdated(
    val usage: UsageSnapshot,
) : AgentEvent

@Serializable
@SerialName("contextCompacted")
data class AgentEventContextCompacted(
    val summary: String,
    val freed: Int,
) : AgentEvent

@Serializable
@SerialName("summaryEdited")
data class AgentEventSummaryEdited(
    val summary: String,
) : AgentEvent
