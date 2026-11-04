@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Commands that act on an MCP or LSP server, named as the session names it.

@Serializable
@SerialName("reconnectMcpServer")
data class CommandReconnectMcpServer(
    val server: String,
) : Command

@Serializable
@SerialName("approveMcpServer")
data class CommandApproveMcpServer(
    val server: String,
) : Command

@Serializable
@SerialName("loginMcpServer")
data class CommandLoginMcpServer(
    val server: String,
) : Command

@Serializable
@SerialName("logoutMcpServer")
data class CommandLogoutMcpServer(
    val server: String,
) : Command

@Serializable
@SerialName("restartLspServer")
data class CommandRestartLspServer(
    val server: String,
) : Command

@Serializable
@SerialName("setLspServerEnabled")
data class CommandSetLspServerEnabled(
    val server: String,
    val enabled: Boolean,
) : Command

@Serializable
@SerialName("installLspServer")
data class CommandInstallLspServer(
    val server: String,
) : Command
