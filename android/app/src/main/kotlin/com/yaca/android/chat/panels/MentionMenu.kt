package com.yaca.android.chat.panels

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.Feed
import com.yaca.core.feed.touchedFiles
import kotlinx.coroutines.delay

/** Quiet time after the last keystroke before a search goes out. */
private const val SEARCH_DEBOUNCE_MS = 150L

/** The longest the list grows before it scrolls: about seven rows. */
private val MENU_MAX_HEIGHT = 280.dp

/**
 * The `@` file menu, shown above the composer. An empty query lists the files this chat has
 * touched; a typed one waits for the core's search for exactly that query, and an answer to an
 * older query is never shown as this one's. A tap on a file picks it; the close button and the
 * system back dismiss the menu without picking anything.
 */
@Composable
fun MentionMenu(
    feed: Feed,
    query: String,
    onSearch: (String) -> Unit,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val searchLatest by rememberUpdatedState(onSearch)
    // A search goes out once the typing pauses. An empty query asks nothing: the touched files are
    // the answer to it, and they are already in the feed.
    LaunchedEffect(query) {
        if (query.isEmpty()) return@LaunchedEffect
        delay(SEARCH_DEBOUNCE_MS)
        searchLatest(query)
    }

    val touched = remember(feed) { touchedFiles(feed) }
    val found = feed.found?.takeIf { it.query == query }
    // `null` is the loading state: the search for this query has not answered yet.
    val paths: List<String>? = if (query.isEmpty()) touched else found?.paths

    BackHandler(onBack = onDismiss)

    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        tonalElevation = 3.dp,
        shadowElevation = 6.dp,
    ) {
        Column {
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = stringResource(
                        if (query.isEmpty()) R.string.mention_recent else R.string.mention_label,
                    ),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.mention_close))
                }
            }
            HorizontalDivider()
            when {
                paths == null -> Row(
                    Modifier.fillMaxWidth().padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(12.dp))
                    Text(
                        text = stringResource(R.string.mention_searching),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                paths.isEmpty() -> Text(
                    text = stringResource(
                        if (query.isEmpty()) R.string.mention_none_touched else R.string.mention_none,
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
                else -> LazyColumn(Modifier.heightIn(max = MENU_MAX_HEIGHT)) {
                    itemsIndexed(paths) { _, path ->
                        Text(
                            text = path,
                            style = LocalCodeStyle.current,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onPick(path) }
                                .padding(horizontal = 16.dp, vertical = 12.dp),
                        )
                    }
                }
            }
        }
    }
}
