@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * How far an "Always allow" grant or an "Always deny" refusal reaches. Session-lived
 * by design: rules written to the project's settings go through the settings screen.
 */
@Serializable
@JsonClassDiscriminator("type")
sealed interface ScopeKey

@Serializable
@SerialName("tool")
data class ScopeKeyTool(
    val name: String,
) : ScopeKey

@Serializable
@SerialName("directory")
data class ScopeKeyDirectory(
    val path: String,
) : ScopeKey

@Serializable
@SerialName("commandPrefix")
data class ScopeKeyCommandPrefix(
    val prefix: String,
) : ScopeKey

@Serializable
@SerialName("domain")
data class ScopeKeyDomain(
    val host: String,
) : ScopeKey

@Serializable
@SerialName("mcpServer")
data class ScopeKeyMcpServer(
    val server: String,
) : ScopeKey

/** The human's answer to a permission request. */
@Serializable
@JsonClassDiscriminator("type")
sealed interface PermissionAnswer

@Serializable
@SerialName("allow")
data object PermissionAnswerAllow : PermissionAnswer

@Serializable
@SerialName("allowAlways")
data class PermissionAnswerAllowAlways(
    val scope: ScopeKey,
) : PermissionAnswer

@Serializable
@SerialName("deny")
data class PermissionAnswerDeny(
    val reason: String? = null,
) : PermissionAnswer

@Serializable
@SerialName("denyAlways")
data class PermissionAnswerDenyAlways(
    val scope: ScopeKey,
    val reason: String? = null,
) : PermissionAnswer
