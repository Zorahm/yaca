package com.yaca.android.nav

import kotlinx.serialization.Serializable

/** The paired computers. The start screen while at least one is paired. */
@Serializable
data object Hosts

/** Pairing a computer: the camera, a pasted link, or a link another app handed over. */
@Serializable
data object Pairing

/** One computer: its connection, its running chats and its projects. */
@Serializable
data class Host(val fingerprint: String)

/** One chat, as a feed. [session] is the host's session id. */
@Serializable
data class Chat(val fingerprint: String, val session: String)

/** The app's settings: the background connection, notifications, colours, and the version. */
@Serializable
data object Settings
