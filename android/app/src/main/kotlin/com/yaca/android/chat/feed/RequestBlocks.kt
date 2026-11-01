package com.yaca.android.chat.feed

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.FeedActions
import com.yaca.android.chat.RequestSheet
import com.yaca.android.chat.markdown.Markdown
import com.yaca.android.chat.sheets.kindLabel
import com.yaca.android.ui.common.WaitingBadge
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.ElicitationStatus
import com.yaca.core.feed.FeedBlock
import com.yaca.core.feed.QuestionStatus
import com.yaca.core.model.QuestionReplyAnswered

// A request in the feed is a line of the conversation. While it waits, it is a card with the way
// to answer it; once answered it is the record of the answer. Each card opens its sheet.

/** An open permission request. The feed hides a closed one, so this is only drawn while it waits. */
@Composable
fun PermissionBlock(
    block: FeedBlock.Permission,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    if (!block.open) return
    val request = block.request
    WaitingCard(modifier = modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = kindLabel(request.kind),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f),
            )
            WaitingBadge()
        }
        Text(
            text = request.target,
            style = LocalCodeStyle.current,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Button(onClick = { actions.onOpenRequest(RequestSheet.Permission(request)) }) {
            Text(stringResource(R.string.permission_block_review))
        }
    }
}

/** A question: open it waits for an answer; afterwards the card shows what was picked, or that it was skipped. */
@Composable
fun QuestionBlock(
    block: FeedBlock.Question,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    if (block.status == QuestionStatus.OPEN) {
        WaitingCard(modifier = modifier) {
            Text(
                text = stringResource(R.string.question_block_title),
                style = MaterialTheme.typography.titleSmall,
            )
            Markdown(text = block.request.question)
            Button(onClick = { actions.onOpenRequest(RequestSheet.Question(block.request)) }) {
                Text(stringResource(R.string.question_answer_action))
            }
        }
        return
    }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(
            text = block.request.question,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = questionOutcome(block),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun questionOutcome(block: FeedBlock.Question): String = when (block.status) {
    QuestionStatus.OPEN -> ""
    QuestionStatus.ANSWERED -> {
        val reply = block.reply as? QuestionReplyAnswered
        if (reply == null) {
            stringResource(R.string.question_unanswered)
        } else {
            val picked = reply.selected.mapNotNull { block.request.options.getOrNull(it)?.label }
            val all = picked + listOfNotNull(reply.other?.takeIf { it.isNotBlank() })
            stringResource(R.string.question_answered, all.joinToString(stringResource(R.string.chat_list_separator)))
        }
    }
    QuestionStatus.DISMISSED -> stringResource(R.string.question_skipped)
    QuestionStatus.UNANSWERED -> stringResource(R.string.question_unanswered)
}

/** A form an MCP server asked for: open it waits for the human; afterwards, what became of it. */
@Composable
fun ElicitationBlock(
    block: FeedBlock.Elicitation,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val request = block.request
    if (block.status == ElicitationStatus.OPEN) {
        WaitingCard(modifier = modifier) {
            Text(
                text = stringResource(R.string.elicit_block_title, request.server),
                style = MaterialTheme.typography.titleSmall,
            )
            Text(text = request.message, style = MaterialTheme.typography.bodyMedium, maxLines = 3, overflow = TextOverflow.Ellipsis)
            Button(onClick = { actions.onOpenRequest(RequestSheet.Elicitation(request)) }) {
                Text(stringResource(R.string.elicit_review))
            }
        }
        return
    }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(
            text = stringResource(R.string.elicit_block_title, request.server),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = stringResource(elicitOutcome(block.status)),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun elicitOutcome(status: ElicitationStatus): Int = when (status) {
    ElicitationStatus.OPEN -> R.string.elicit_review
    ElicitationStatus.ACCEPTED -> R.string.elicit_sent
    ElicitationStatus.DECLINED -> R.string.elicit_declined
    ElicitationStatus.CANCELLED -> R.string.elicit_cancelled
    ElicitationStatus.EXPIRED -> R.string.elicit_expired
}

/** The card an open request is drawn in: the accent border, since it is what needs the human. */
@Composable
private fun WaitingCard(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.primary),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            content()
        }
    }
}
