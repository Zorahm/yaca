package com.yaca.core.client

/** The host's messages as the Rust side writes them, for scripting a [FakeHost] connection. */
internal object Msg {
    val turnStarted = """{"type":"turnStarted","turnId":"t1","at":1}"""

    fun textDelta(text: String) = """{"type":"textDelta","turnId":"t1","blockId":"b1","text":"$text"}"""

    fun welcome(name: String = "Desk") =
        """{"type":"welcome","host":{"name":"$name","version":"1.0.0-beta","protocol":1}}"""

    fun refused(kind: String) = """{"type":"refused","reason":{"kind":"$kind"}}"""

    fun paired(token: String) =
        """{"type":"paired","deviceId":"device-2","token":"$token",""" +
            """"host":{"name":"Desk","version":"1.0.0-beta","protocol":1}}"""

    fun response(id: Long, reply: String) =
        """{"type":"response","id":$id,"result":{"status":"ok","reply":$reply}}"""

    fun errorResponse(id: Long, code: String, message: String) =
        """{"type":"response","id":$id,"result":{"status":"error","error":{"code":"$code","message":"$message"}}}"""

    fun projects(id: Long, vararg names: String): String {
        val list = names.joinToString(",") { "\"$it\"" }
        return response(id, """{"kind":"projects","projects":[$list]}""")
    }

    fun done(id: Long) = response(id, """{"kind":"done"}""")

    fun attached(id: Long, session: String, epoch: Int, events: List<String>): String =
        response(id, """{"kind":"attached","session":"$session","epoch":$epoch,"events":[${events.joinToString(",")}]}""")

    fun frame(session: String, epoch: Int, events: List<String>) =
        """{"type":"frame","session":"$session","epoch":$epoch,"events":[${events.joinToString(",")}]}"""

    fun resync(session: String) = """{"type":"resync","session":"$session"}"""

    fun chatsChanged(vararg ids: String): String {
        val chats = ids.joinToString(",") { chat(it) }
        return """{"type":"chatsChanged","chats":[$chats]}"""
    }

    private fun chat(id: String) =
        """{"id":"$id","project":"/p","state":{"projectRoot":"/p","provider":"","model":"m",""" +
            """"mode":"auto","effort":"medium","busy":false,"bypass":false}}"""
}
