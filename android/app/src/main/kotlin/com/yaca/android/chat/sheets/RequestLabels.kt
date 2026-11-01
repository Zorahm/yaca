package com.yaca.android.chat.sheets

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import com.yaca.core.model.Invariant
import com.yaca.core.model.PermissionKind
import com.yaca.core.model.ScopeKey
import com.yaca.core.model.ScopeKeyCommandPrefix
import com.yaca.core.model.ScopeKeyDirectory
import com.yaca.core.model.ScopeKeyDomain
import com.yaca.core.model.ScopeKeyMcpServer
import com.yaca.core.model.ScopeKeyTool

/** What is being asked for, in words. */
@Composable
fun kindLabel(kind: PermissionKind): String = stringResource(
    when (kind) {
        PermissionKind.RUN_COMMAND -> R.string.kind_run_command
        PermissionKind.DELETE_FILE -> R.string.kind_delete_file
        PermissionKind.DELETE_DIRECTORY -> R.string.kind_delete_directory
        PermissionKind.MOVE_FILE -> R.string.kind_move_file
        PermissionKind.EDIT_FILE -> R.string.kind_edit_file
        PermissionKind.FETCH_URL -> R.string.kind_fetch_url
        PermissionKind.MCP_WRITE -> R.string.kind_mcp_write
    },
)

/** Why a question no rule can settle is always asked. */
@Composable
fun invariantLabel(invariant: Invariant): String = stringResource(
    when (invariant) {
        Invariant.GIT_COMMIT -> R.string.invariant_git_commit
        Invariant.GIT_PUSH -> R.string.invariant_git_push
        Invariant.GIT_UNRESOLVED -> R.string.invariant_git_unresolved
        Invariant.GIT_CONFIG -> R.string.invariant_git_config
    },
)

/** The reach of an "always" choice: the tool, the folder, the command prefix, the domain or the server. */
@Composable
fun scopeLabel(scope: ScopeKey): String = when (scope) {
    is ScopeKeyTool -> stringResource(R.string.scope_tool, scope.name)
    is ScopeKeyDirectory -> stringResource(R.string.scope_directory, scope.path)
    is ScopeKeyCommandPrefix -> stringResource(R.string.scope_command_prefix, scope.prefix)
    is ScopeKeyDomain -> stringResource(R.string.scope_domain, scope.host)
    is ScopeKeyMcpServer -> stringResource(R.string.scope_mcp_server, scope.server)
}
