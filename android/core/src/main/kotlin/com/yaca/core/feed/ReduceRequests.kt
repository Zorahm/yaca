package com.yaca.core.feed

import com.yaca.core.model.AgentEventElicitationRequested
import com.yaca.core.model.AgentEventElicitationResolved
import com.yaca.core.model.AgentEventPermissionRequested
import com.yaca.core.model.AgentEventPermissionResolved
import com.yaca.core.model.AgentEventQuestionAsked
import com.yaca.core.model.AgentEventQuestionResolved
import com.yaca.core.model.ElicitationReply
import com.yaca.core.model.ElicitationReplyAccept
import com.yaca.core.model.ElicitationReplyCancel
import com.yaca.core.model.ElicitationReplyDecline
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.QuestionReplyAnswered
import com.yaca.core.model.QuestionReplyDismissed
import com.yaca.core.model.RequestId

// Things the human is asked for: permission requests, questions, and MCP server forms.
// Each request is one block that is closed in place when its answer arrives.

internal fun FeedDraft.onPermissionRequested(event: AgentEventPermissionRequested) {
    request = event.request
    blocks.add(FeedBlock.Permission(key("ask"), event.request, open = true))
}

internal fun FeedDraft.onPermissionResolved(event: AgentEventPermissionResolved) {
    request = null
    closeRequest(event.requestId)
}

internal fun FeedDraft.onQuestionAsked(event: AgentEventQuestionAsked) {
    question = event.request
    blocks.add(FeedBlock.Question(key("ques"), event.request, QuestionStatus.OPEN, reply = null))
}

internal fun FeedDraft.onQuestionResolved(event: AgentEventQuestionResolved) {
    if (question?.id == event.requestId) question = null
    settleQuestion(event.requestId, event.reply)
}

/**
 * A window that reconnects is sent the open forms again: one block per form, whichever way it
 * arrived.
 */
internal fun FeedDraft.onElicitationRequested(event: AgentEventElicitationRequested) {
    if (blocks.any { it is FeedBlock.Elicitation && it.request.id == event.request.id }) return
    blocks.add(FeedBlock.Elicitation(key("elicit"), event.request, ElicitationStatus.OPEN))
}

internal fun FeedDraft.onElicitationResolved(event: AgentEventElicitationResolved) {
    settleElicitation(event.requestId, event.reply)
}

/** Marks the permission block closed. Its row stays in the list: blocks are only ever added. */
private fun FeedDraft.closeRequest(requestId: RequestId) {
    val at = blocks.indexOfLast { it is FeedBlock.Permission && it.request.id == requestId }
    val block = blocks.getOrNull(at) as? FeedBlock.Permission ?: return
    blocks[at] = block.copy(open = false)
}

/**
 * Closes the question with an answer, a refusal, or nothing.
 *
 * Writes to the same block rather than adding a new one: there is one question, and its row in
 * the feed should become the outcome, not a second entry about the same thing.
 */
private fun FeedDraft.settleQuestion(requestId: RequestId, reply: QuestionReply?) {
    val at = blocks.indexOfLast { it is FeedBlock.Question && it.request.id == requestId }
    val block = blocks.getOrNull(at) as? FeedBlock.Question ?: return
    val status = when (reply) {
        null -> QuestionStatus.UNANSWERED
        is QuestionReplyDismissed -> QuestionStatus.DISMISSED
        is QuestionReplyAnswered -> QuestionStatus.ANSWERED
    }
    blocks[at] = block.copy(status = status, reply = reply)
}

private fun FeedDraft.settleElicitation(requestId: RequestId, reply: ElicitationReply?) {
    val at = blocks.indexOfLast { it is FeedBlock.Elicitation && it.request.id == requestId }
    val block = blocks.getOrNull(at) as? FeedBlock.Elicitation ?: return
    // `null`: the server stopped waiting before anyone answered.
    val status = when (reply) {
        null -> ElicitationStatus.EXPIRED
        is ElicitationReplyAccept -> ElicitationStatus.ACCEPTED
        is ElicitationReplyDecline -> ElicitationStatus.DECLINED
        is ElicitationReplyCancel -> ElicitationStatus.CANCELLED
    }
    blocks[at] = block.copy(status = status)
}
