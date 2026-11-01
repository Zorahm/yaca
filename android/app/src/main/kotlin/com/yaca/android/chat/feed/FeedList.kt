package com.yaca.android.chat.feed

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.ChatUi
import com.yaca.android.chat.FeedActions
import com.yaca.android.chat.formatDuration
import com.yaca.android.ui.common.BusyDot
import com.yaca.core.feed.FeedItem
import com.yaca.core.feed.isLiveWork
import kotlinx.coroutines.delay

/**
 * The feed as a list, newest at the bottom. The list is laid out reversed: the newest item sits at
 * the bottom edge and the list stays anchored there, so a reader at the bottom follows new content
 * by itself, while one who scrolled up keeps their place and nothing moves under them. That is the
 * whole auto-scroll rule; no scroll is issued from here.
 */
@Composable
fun FeedList(
    ui: ChatUi,
    listState: LazyListState,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val items = ui.items
    val busy = ui.feed.state?.busy == true
    // A turn that is running and has no work group at its end still runs: the row says so. A group
    // at the end pulses by itself, so no extra row is needed then.
    val showWorking = busy && items.lastOrNull() !is FeedItem.Work
    LazyColumn(
        state = listState,
        reverseLayout = true,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (showWorking) {
            item(key = "working_row") { WorkingRow(startedAt = ui.feed.turn?.startedAt?.value) }
        }
        if (items.isEmpty() && !busy) {
            item(key = "empty_chat") { EmptyChat() }
        }
        itemsIndexed(items.asReversed(), key = { _, item -> item.key }) { reversed, item ->
            val index = items.lastIndex - reversed
            FeedItemView(item = item, live = isLiveWork(items, index, busy), actions = actions)
        }
    }
}

/** A running turn, with its elapsed time counted from the core's own start stamp. */
@Composable
private fun WorkingRow(startedAt: Long?) {
    val now by produceState(initialValue = System.currentTimeMillis(), key1 = startedAt) {
        while (true) {
            value = System.currentTimeMillis()
            delay(1_000L)
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        BusyDot(busy = true)
        Spacer(Modifier.width(8.dp))
        val text = if (startedAt != null && startedAt > 0L) {
            stringResource(R.string.chat_working_for, formatDuration(now - startedAt))
        } else {
            stringResource(R.string.chat_working)
        }
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun EmptyChat() {
    Text(
        text = stringResource(R.string.chat_empty),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth().padding(vertical = 24.dp),
    )
}
