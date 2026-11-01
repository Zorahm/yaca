package com.yaca.core.feed

/**
 * The work of a turn, folded into feed items, for display: the reducer keeps blocks in the order
 * things happened, and a thought, two calls, another thought, three more calls would be a ladder of
 * one-line groups. Here every stretch of calls and thoughts becomes one [FeedItem.Work], in order.
 *
 * What the agent says divides the work: a line of text between calls stands in the feed as text,
 * with the work before it above and the work after it below. Nothing else the agent does on its own
 * cuts a stretch: a thought least of all, since a model that thinks before every call would be back
 * to the ladder.
 *
 * Sending subagents divides the work too. The `Task` calls of one stretch leave it and stand in the
 * feed as one [FeedItem.Agents]: what a subagent does is read in the agents panel, and a row buried
 * in a folded card would hide that several agents are at work right now.
 *
 * It is a view over the blocks, not a second state: the reducer's list still only grows, and this is
 * recomputed from it on every frame.
 */
fun fold(blocks: List<FeedBlock>): List<FeedItem> {
    val out = mutableListOf<FeedItem>()
    var at = 0
    while (at < blocks.size) {
        var end = at
        while (end < blocks.size && isInRun(blocks[end])) end += 1
        if (end == at) {
            pass(out, blocks[at])
            at += 1
            continue
        }
        foldRun(out, blocks.subList(at, end))
        at = end
    }
    return out
}

/**
 * Blocks a stretch of work may run through without being cut in two: reasoning, actions, the todo
 * list and checkpoints.
 */
private fun isInRun(block: FeedBlock): Boolean =
    block is FeedBlock.Reasoning ||
        block is FeedBlock.Actions ||
        block is FeedBlock.Todos ||
        block is FeedBlock.Checkpoint

private fun isWork(block: FeedBlock): Boolean =
    block is FeedBlock.Reasoning || block is FeedBlock.Actions

private fun foldRun(out: MutableList<FeedItem>, run: List<FeedBlock>) {
    // A checkpoint or the todo list at either end is not inside the work.
    val first = run.indexOfFirst(::isWork)
    if (first == -1) {
        for (block in run) pass(out, block)
        return
    }
    val last = run.indexOfLast(::isWork)

    for (block in run.subList(0, first)) pass(out, block)

    val steps = mutableListOf<FeedItem.WorkStep>()
    val agents = mutableListOf<ActionRow>()
    // Each piece is keyed by what it starts with, so it keeps its identity while it grows.
    fun flushSteps() {
        if (steps.isNotEmpty()) out.add(FeedItem.Work("work_${steps[0].key}", steps.toList()))
        steps.clear()
    }
    fun flushAgents() {
        if (agents.isNotEmpty()) out.add(FeedItem.Agents("agents_${agents[0].callId.value}", agents.toList()))
        agents.clear()
    }
    fun step(next: FeedItem.WorkStep) {
        flushAgents()
        steps.add(next)
    }

    // The todo list is not a step: it is one block for the whole session, updated in place. It
    // stands right under the work it appeared in.
    val after = mutableListOf<FeedBlock>()
    for (block in run.subList(first, last + 1)) {
        when (block) {
            is FeedBlock.Reasoning -> step(FeedItem.WorkStep.Thought(block.key, block.text))
            is FeedBlock.Actions -> {
                for (row in block.rows) {
                    if (isAgent(row)) {
                        flushSteps()
                        agents.add(row)
                    } else {
                        step(FeedItem.WorkStep.Action(row.callId.value, row))
                    }
                }
            }
            is FeedBlock.Checkpoint -> step(FeedItem.WorkStep.Checkpoint(block.key, block))
            else -> after.add(block)
        }
    }
    flushSteps()
    flushAgents()

    for (block in after) pass(out, block)
    for (block in run.subList(last + 1, run.size)) pass(out, block)
}

/**
 * Passes a block through unchanged. Reasoning and actions never get here: a run that holds either
 * always folds it, so the `is FeedItem` test is the one that matters for the rest.
 */
private fun pass(out: MutableList<FeedItem>, block: FeedBlock) {
    if (block is FeedItem) out.add(block)
}
