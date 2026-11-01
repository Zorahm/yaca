package com.yaca.android.host

import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import com.yaca.android.ui.common.CenteredProgress
import com.yaca.android.ui.common.MutedText
import com.yaca.android.ui.common.projectShortName

/** The projects, each followed by its chats when it is open. Flattened into the list, so the whole list scrolls as one. */
internal fun LazyListScope.projectItems(
    ready: HostUiState.Ready,
    onToggleProject: (String) -> Unit,
    onOpenRecorded: (String, String) -> Unit,
) {
    when (val projects = ready.projects) {
        ProjectsState.Loading -> item { CenteredProgress() }
        ProjectsState.Unavailable -> item { MutedText(stringResource(R.string.host_projects_unavailable)) }
        is ProjectsState.Loaded -> if (projects.projects.isEmpty()) {
            item { MutedText(stringResource(R.string.host_no_projects)) }
        } else {
            projects.projects.forEach { project ->
                projectRows(project, ready.expanded[project], onToggleProject, onOpenRecorded)
            }
        }
    }
}

private fun LazyListScope.projectRows(
    project: String,
    sessions: SessionsState?,
    onToggleProject: (String) -> Unit,
    onOpenRecorded: (String, String) -> Unit,
) {
    item(key = "project:$project") {
        ProjectHeaderRow(
            name = projectShortName(project),
            expanded = sessions != null,
            chatCount = (sessions as? SessionsState.Loaded)?.sessions?.size,
            onClick = { onToggleProject(project) },
        )
    }
    when (sessions) {
        null -> Unit
        SessionsState.Loading -> item(key = "loading:$project") { CenteredProgress() }
        SessionsState.Unavailable -> item(key = "unavailable:$project") {
            MutedText(stringResource(R.string.host_chats_unavailable))
        }
        is SessionsState.Loaded -> if (sessions.sessions.isEmpty()) {
            item(key = "empty:$project") { MutedText(stringResource(R.string.host_no_chats)) }
        } else {
            items(sessions.sessions, key = { it.id.value }) { meta ->
                RecordedChatRow(meta = meta, onClick = { onOpenRecorded(project, meta.id.value) })
            }
        }
    }
}

