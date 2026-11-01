package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Which file an MCP server was declared in. */
@Serializable
enum class McpSource {
    @SerialName("global") GLOBAL,
    @SerialName("projectFile") PROJECT_FILE,
    @SerialName("project") PROJECT,
    @SerialName("plugin") PLUGIN,
    @SerialName("app") APP,
}

@Serializable
enum class McpTransport {
    @SerialName("stdio") STDIO,
    @SerialName("http") HTTP,
    @SerialName("app") APP,
}

/** What state a server is in. The interface draws its list from this and nothing else. */
@Serializable
enum class McpStatus {
    @SerialName("connecting") CONNECTING,
    @SerialName("ready") READY,
    @SerialName("failed") FAILED,
    @SerialName("disabled") DISABLED,
    @SerialName("needsApproval") NEEDS_APPROVAL,
    @SerialName("needsLogin") NEEDS_LOGIN,
    @SerialName("loggingIn") LOGGING_IN,
}

@Serializable
data class McpToolSummary(
    /** The tool's own name, as the server calls it. */
    val name: String,
    val description: String? = null,
    /** The server marks it as changing nothing, so it runs without a question. */
    val readOnly: Boolean,
)

/** A server's prompt template, offered by the slash menu. */
@Serializable
data class McpPromptSummary(
    /** What is typed after the slash: `mcp__server__prompt`. */
    val command: String,
    val description: String? = null,
    /** The arguments, in the order the words typed after the name fill them. */
    val arguments: List<String>,
)

@Serializable
data class McpServerSummary(
    val name: String,
    val source: McpSource,
    val transport: McpTransport,
    /** The command line or the address, as declared. */
    val target: String,
    val status: McpStatus,
    val reason: String? = null,
    /** How the server introduces itself: `name version`. */
    val label: String? = null,
    val tools: List<McpToolSummary>,
    /** The server exposes resources, so the two resource tools reach it. */
    val resources: Boolean,
    val prompts: List<McpPromptSummary>,
    /** While logging in: the page to open in the browser. */
    val loginUrl: String? = null,
    /** Yaca holds an OAuth login for this server, which "Log out" forgets. */
    val loggedIn: Boolean,
)
