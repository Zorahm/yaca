@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import com.yaca.core.model.Command
import com.yaca.core.model.SessionId
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * What a request asks for, discriminated by `kind`. Each call gets one [Reply] or
 * [CallError] in the matching response.
 */
@Serializable
@JsonClassDiscriminator("kind")
sealed interface Call

/** The host's projects, most recent first. */
@Serializable
@SerialName("listProjects")
data object CallListProjects : Call

/** The recorded chats of a project. */
@Serializable
@SerialName("listSessions")
data class CallListSessions(
    val project: String,
) : Call

/** Every model the host's providers offer, for the model picker. */
@Serializable
@SerialName("listModels")
data object CallListModels : Call

/** The chats running on the host right now. */
@Serializable
@SerialName("listChats")
data object CallListChats : Call

/** The feed of a recorded chat that is not running, read from its journal. */
@Serializable
@SerialName("sessionFeed")
data class CallSessionFeed(
    val project: String,
    val session: SessionId,
) : Call

/**
 * Starts a chat: a new one, or a recorded one when [resume] names it. Opening a chat
 * that is already running hands back the running one. A null [startup] leaves the
 * choice to the project's settings, as the host does for a chat opened there.
 */
@Serializable
@SerialName("openChat")
data class CallOpenChat(
    val project: String,
    val resume: SessionId? = null,
    val startup: Startup? = null,
) : Call

/**
 * Starts sending the chat's events. The reply is the whole feed so far; frames with
 * the same epoch follow. Attaching again starts a new epoch, and frames of the older
 * one are dropped.
 */
@Serializable
@SerialName("attach")
data class CallAttach(
    val session: SessionId,
) : Call

/** Stops sending the chat's events. The chat goes on running. */
@Serializable
@SerialName("detach")
data class CallDetach(
    val session: SessionId,
) : Call

/** Stops the turn and closes the chat on the host, for every client. The journal stays. */
@Serializable
@SerialName("closeChat")
data class CallCloseChat(
    val session: SessionId,
) : Call

/** A core command for a running chat. Its effect arrives as events of an attached chat. */
@Serializable
@SerialName("send")
data class CallSend(
    val session: SessionId,
    val command: Command,
) : Call
