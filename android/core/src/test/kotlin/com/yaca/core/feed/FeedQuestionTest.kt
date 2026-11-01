package com.yaca.core.feed

import com.yaca.core.model.AgentEventQuestionAsked
import com.yaca.core.model.AgentEventQuestionResolved
import com.yaca.core.model.CallId
import com.yaca.core.model.QuestionOption
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.QuestionReplyAnswered
import com.yaca.core.model.QuestionReplyDismissed
import com.yaca.core.model.QuestionRequest
import com.yaca.core.model.RequestId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull

/** Ports of the `AskUser questions` group of `src/core/feed.test.ts`. */
class FeedQuestionTest {
    private val request = QuestionRequest(
        id = RequestId("q_1"),
        callId = CallId("c1"),
        question = "Which path?",
        options = listOf(
            QuestionOption(label = "Quick", description = "without tests"),
            QuestionOption(label = "Full", description = null),
        ),
        allowOther = true,
        multiSelect = false,
    )

    private val asked = AgentEventQuestionAsked(request)

    private fun resolved(reply: QuestionReply?) = AgentEventQuestionResolved(RequestId("q_1"), reply)

    private fun question(feed: Feed): FeedBlock.Question =
        assertIs<FeedBlock.Question>(feed.blocks.first { it is FeedBlock.Question })

    @Test
    fun `a question appears in the feed open and takes the bottom row`() {
        val feed = runFeed(asked)

        assertEquals(RequestId("q_1"), feed.question?.id)
        assertEquals(QuestionStatus.OPEN, question(feed).status)
        assertNull(question(feed).reply)
        assertEquals(1, visible(feed).size)
    }

    @Test
    fun `an answer closes the question, but the row with the choice stays in the feed`() {
        val answer = QuestionReplyAnswered(selected = listOf(1), other = null)
        val feed = runFeed(asked, resolved(answer))

        assertNull(feed.question)
        assertEquals(QuestionStatus.ANSWERED, question(feed).status)
        assertEquals(answer, question(feed).reply)
        // Blocks only get added: the outcome of an answer is part of the conversation.
        assertEquals(1, visible(feed).size)
    }

    @Test
    fun `a refusal to answer is marked apart from an answer`() {
        val feed = runFeed(asked, resolved(QuestionReplyDismissed))

        assertNull(feed.question)
        assertEquals(QuestionStatus.DISMISSED, question(feed).status)
        assertEquals(QuestionReplyDismissed, question(feed).reply)
    }

    @Test
    fun `an answer to a question not on screen touches nothing`() {
        val feed = runFeed(
            asked,
            AgentEventQuestionResolved(RequestId("other"), QuestionReplyDismissed),
        )

        assertEquals(RequestId("q_1"), feed.question?.id)
        assertEquals(QuestionStatus.OPEN, question(feed).status)
    }

    @Test
    fun `a question with no answer from the journal is closed, not left open`() {
        // This is how a replay of an unfinished run looks: the core closes the question without an
        // answer, and the feed draws no buttons that nobody listens to any more.
        val feed = runFeed(asked, resolved(null))

        assertNull(feed.question)
        assertEquals(QuestionStatus.UNANSWERED, question(feed).status)
        assertNull(question(feed).reply)
    }

    @Test
    fun `a choice keeps its own text next to the ticked options`() {
        val answer = QuestionReplyAnswered(selected = listOf(0), other = "both at once")
        val feed = runFeed(asked, resolved(answer))

        assertEquals(answer, question(feed).reply)
    }
}
