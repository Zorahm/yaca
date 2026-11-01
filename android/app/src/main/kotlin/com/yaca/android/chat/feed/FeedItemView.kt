package com.yaca.android.chat.feed

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.yaca.android.chat.FeedActions
import com.yaca.core.feed.FeedBlock
import com.yaca.core.feed.FeedItem

/**
 * One entry of the feed, by its kind. The `when` has no `else`: a kind the feed gains is a compile
 * error here, not a silently dropped line. Reasoning and actions never reach this point; they are
 * drawn inside their work group.
 */
@Composable
fun FeedItemView(
    item: FeedItem,
    live: Boolean,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    when (item) {
        is FeedItem.Work -> WorkGroup(work = item, live = live, actions = actions, modifier = modifier)
        is FeedItem.Agents -> AgentsGroup(agents = item, actions = actions, modifier = modifier)
        is FeedBlock.Human -> UserBlock(block = item, modifier = modifier)
        is FeedBlock.Agent -> TextBlock(text = item.text, modifier = modifier)
        is FeedBlock.Todos -> TodosCard(items = item.items, modifier = modifier)
        is FeedBlock.Permission -> PermissionBlock(block = item, actions = actions, modifier = modifier)
        is FeedBlock.Question -> QuestionBlock(block = item, actions = actions, modifier = modifier)
        is FeedBlock.Elicitation -> ElicitationBlock(block = item, actions = actions, modifier = modifier)
        is FeedBlock.CheckpointCreating -> CheckpointCreatingLine(modifier)
        is FeedBlock.Checkpoint -> CheckpointDivider(checkpoint = item.checkpoint, onOpen = actions.onOpenCheckpoints, modifier = modifier)
        is FeedBlock.Restored -> RestoredDivider(block = item, modifier = modifier)
        is FeedBlock.Compacted -> CompactedCard(block = item, modifier = modifier)
        is FeedBlock.Plan -> PlanCard(plan = item.plan, modifier = modifier)
        is FeedBlock.Shown -> ShownCard(block = item, modifier = modifier)
        is FeedBlock.Error -> ErrorCard(block = item, modifier = modifier)
        is FeedBlock.Retry -> RetryLine(block = item, modifier = modifier)
        is FeedBlock.Route -> RouteLine(block = item, modifier = modifier)
        is FeedBlock.Stopped -> StoppedLine(modifier = modifier)
        is FeedBlock.Hook -> HookLine(block = item, modifier = modifier)
        is FeedBlock.TurnTime -> TurnTimeLine(block = item, modifier = modifier)
    }
}
