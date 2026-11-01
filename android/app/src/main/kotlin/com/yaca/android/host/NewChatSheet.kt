@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.host

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.common.CenteredProgress
import com.yaca.android.ui.common.MutedText
import com.yaca.android.ui.common.projectShortName

/**
 * "New chat": pick the project the chat starts in. Choosing one opens the chat on the host; the
 * sheet stays up, with a progress line, until the host has answered.
 */
@Composable
fun NewChatSheet(
    projects: ProjectsState,
    opening: Boolean,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
            Text(stringResource(R.string.host_pick_project), style = MaterialTheme.typography.titleMedium)
            when (projects) {
                ProjectsState.Loading -> CenteredProgress()
                ProjectsState.Unavailable -> MutedText(stringResource(R.string.host_projects_unavailable))
                is ProjectsState.Loaded -> if (projects.projects.isEmpty()) {
                    MutedText(stringResource(R.string.host_no_projects))
                } else {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        projects.projects.forEach { project ->
                            ProjectPickRow(
                                project = project,
                                enabled = !opening,
                                onClick = { onPick(project) },
                            )
                        }
                    }
                }
            }
            if (opening) {
                Text(
                    text = stringResource(R.string.host_opening),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp),
                )
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
            }
        }
    }
}

@Composable
private fun ProjectPickRow(project: String, enabled: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled, onClick = onClick)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Folder, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Column(Modifier.padding(start = 12.dp)) {
            Text(
                text = projectShortName(project),
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = project,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
