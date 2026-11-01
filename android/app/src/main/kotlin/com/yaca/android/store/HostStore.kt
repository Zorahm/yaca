package com.yaca.android.store

import android.content.Context
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.yaca.core.client.HostRecord
import com.yaca.core.protocol.YacaJson
import java.io.IOException
import java.security.GeneralSecurityException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

private val Context.hostsDataStore: DataStore<Preferences> by preferencesDataStore(name = "yaca_hosts")
private val HOSTS_KEY = stringPreferencesKey("hosts.v1")
private const val TAG = "HostStore"

/**
 * The paired hosts, as one encrypted string in DataStore. The JSON is sealed by [HostCipher]
 * with a key that never leaves the Android Keystore, so the preferences file alone reveals
 * no tokens.
 */
class HostStore(context: Context) {
    private val dataStore = context.applicationContext.hostsDataStore

    /**
     * Every paired host. A store that cannot be decrypted (the Keystore key was lost, for
     * example after a restore) reads as empty and logs a warning; it never throws.
     */
    val hosts: Flow<List<HostRecord>> = dataStore.data
        .catch { cause ->
            if (cause !is IOException) throw cause
            Log.w(TAG, "The host store could not be read; starting with none (${cause.javaClass.simpleName})")
            emit(emptyPreferences())
        }
        .map { prefs -> decode(prefs[HOSTS_KEY]) }
        .distinctUntilChanged()

    /** Adds [record], replacing the one with the same fingerprint (the host was paired again). */
    suspend fun add(record: HostRecord) {
        replaceAll { hosts -> hosts.filterNot { it.fingerprint == record.fingerprint } + record }
    }

    /** Forgets the host: its credentials are gone from the phone. */
    suspend fun remove(fingerprint: String) {
        replaceAll { hosts -> hosts.filterNot { it.fingerprint == fingerprint } }
    }

    private suspend fun replaceAll(change: (List<HostRecord>) -> List<HostRecord>) {
        dataStore.edit { prefs ->
            val current = decode(prefs[HOSTS_KEY])
            prefs[HOSTS_KEY] = HostCipher.encrypt(YacaJson.encodeToString(change(current)))
        }
    }

    /**
     * Decodes the stored value. Undecryptable or malformed data yields no hosts: the next write
     * then overwrites it, which is the only way forward once the key is gone.
     */
    private fun decode(stored: String?): List<HostRecord> {
        if (stored == null) return emptyList()
        return try {
            YacaJson.decodeFromString<List<HostRecord>>(HostCipher.decrypt(stored))
        } catch (failure: GeneralSecurityException) {
            warnUnreadable(failure)
            emptyList()
        } catch (failure: IllegalArgumentException) {
            // Malformed Base64 or JSON; kotlinx's SerializationException is an IllegalArgumentException.
            warnUnreadable(failure)
            emptyList()
        }
    }

    private fun warnUnreadable(failure: Exception) {
        Log.w(TAG, "Paired computers could not be read; starting with none (${failure.javaClass.simpleName})")
    }
}
