package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** What is being asked for. The interface maps it onto its wording table. */
@Serializable
enum class PermissionKind {
    @SerialName("runCommand") RUN_COMMAND,
    @SerialName("deleteFile") DELETE_FILE,
    @SerialName("deleteDirectory") DELETE_DIRECTORY,
    @SerialName("moveFile") MOVE_FILE,
    @SerialName("editFile") EDIT_FILE,
    @SerialName("fetchUrl") FETCH_URL,
    @SerialName("mcpWrite") MCP_WRITE,
}

/**
 * A question no standing rule and no bypass can settle: a commit, a push, or a
 * write into git's configuration. The host offers no "always" for these.
 */
@Serializable
enum class Invariant {
    @SerialName("gitCommit") GIT_COMMIT,
    @SerialName("gitPush") GIT_PUSH,
    @SerialName("gitUnresolved") GIT_UNRESOLVED,
    @SerialName("gitConfig") GIT_CONFIG,
}

/** Files inside a directory, counted for the question about deleting it. */
@Serializable
data class FileCount(
    val files: Int,
    /** Counting stopped at [files] and there are more. */
    val more: Boolean,
)

/**
 * What a directory deletion takes away that a rollback cannot bring back. The
 * shadow repository keeps neither skipped directories (build output, caches) nor
 * files it cannot track.
 */
@Serializable
data class Unsaved(
    /** Project-relative paths of the skipped directories: the first few. */
    val dirs: List<String>,
    /** How many skipped directories there are in all. */
    val dirCount: Int,
    /** Files the snapshot does not keep. */
    val files: Int,
    /** Additional roots the action reaches; nothing in them comes back on a rollback. */
    val roots: List<String> = emptyList(),
)

/**
 * A live question shown as an inline block over the feed. The optional fields
 * are absent from journals written before they existed, so they default to null.
 */
@Serializable
data class PermissionRequest(
    val id: RequestId,
    val callId: CallId,
    val kind: PermissionKind,
    /** The short target: a file name, the first word of the command, a domain. */
    val target: String,
    /** The full target, monospaced and never truncated; this is where `&& rm -rf` shows. */
    val detail: String,
    /** The dimmed line under the detail: the directory a command would run in. */
    val note: String? = null,
    /** Size of the file the action touches, in bytes. */
    val sizeBytes: Long? = null,
    val files: FileCount? = null,
    val invariant: Invariant? = null,
    val unsaved: Unsaved? = null,
    /** The MCP server the call goes to, when the action is an MCP call. */
    val server: String? = null,
    /** Choices offered behind "Always allow" and "Always deny" alike; empty for an invariant. */
    val scopes: List<ScopeKey>,
    /** The "2 of 3" in the corner of the block. */
    val position: Int,
    val total: Int,
)
