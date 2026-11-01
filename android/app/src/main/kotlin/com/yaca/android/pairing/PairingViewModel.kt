package com.yaca.android.pairing

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.yaca.android.AppContainer
import com.yaca.core.client.HostRecord
import com.yaca.core.client.PairingLink
import com.yaca.core.client.PairingRefusedException
import com.yaca.core.client.Pairer
import java.io.IOException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch

/**
 * The pairing flow. Any valid link pairs: a scanned or pasted one at once, because the human
 * is pointing at the computer; a link from another app only after a confirmation, since any app
 * can open `yaca://pair`.
 */
class PairingViewModel(
    private val container: AppContainer,
    private val deviceName: String,
) : ViewModel() {
    private val mutableState = MutableStateFlow<PairingUiState>(PairingUiState.Scan())
    val state: StateFlow<PairingUiState> = mutableState.asStateFlow()

    /** The one pairing in flight. A second code is ignored until it ends. */
    private var inFlight: Job? = null

    private val busy: Boolean get() = inFlight?.isActive == true

    init {
        viewModelScope.launch {
            container.incomingLink.filterNotNull().collect {
                val link = container.takeIncomingLink() ?: return@collect
                offerIncoming(link)
            }
        }
    }

    /** A value the camera read. Anything that is not a pairing link is someone else's QR code: ignored. */
    fun onScanned(raw: String) {
        if (busy) return
        val link = PairingLink.parse(raw) ?: return
        pair(link)
    }

    /** The clipboard's text, when the human asks for it. */
    fun onPasted(raw: String?) {
        if (busy) return
        val link = raw?.let { PairingLink.parse(it) }
        if (link == null) {
            mutableState.value = PairingUiState.Scan(PairingFailure.InvalidLink)
        } else {
            pair(link)
        }
    }

    fun confirmIncoming() {
        val pending = mutableState.value as? PairingUiState.Confirm ?: return
        val link = PairingLink.parse(pending.link)
        if (link == null) {
            mutableState.value = PairingUiState.Scan(PairingFailure.InvalidLink)
        } else {
            pair(link)
        }
    }

    fun cancelIncoming() {
        if (mutableState.value is PairingUiState.Confirm) mutableState.value = PairingUiState.Scan()
    }

    private fun offerIncoming(raw: String) {
        if (busy) return
        val link = PairingLink.parse(raw)
        mutableState.value = if (link == null) {
            PairingUiState.Scan(PairingFailure.InvalidLink)
        } else {
            PairingUiState.Confirm(computerName = link.name, link = raw)
        }
    }

    private fun pair(link: PairingLink) {
        mutableState.value = PairingUiState.Pairing(link.name)
        inFlight = viewModelScope.launch {
            val record: HostRecord = try {
                Pairer.pair(link, deviceName)
            } catch (refused: PairingRefusedException) {
                finish(PairingUiState.Scan(PairingFailure.Refused(refused.reason)))
                return@launch
            } catch (unreachable: IOException) {
                finish(PairingUiState.Scan(PairingFailure.Network))
                return@launch
            }
            try {
                container.hostStore.add(record)
            } catch (unsaved: CancellationException) {
                throw unsaved
            } catch (unsaved: Exception) {
                // A disk error or a Keystore that refused the key: both lose the credentials.
                // The host has registered a device whose token now exists nowhere; it shows
                // in the host's list and can be revoked there. Pairing again is the way on.
                finish(PairingUiState.Scan(PairingFailure.Storage))
                return@launch
            }
            container.clientFor(record)
            finish(PairingUiState.Paired(record.fingerprint))
        }
    }

    private fun finish(next: PairingUiState) {
        inFlight = null
        mutableState.value = next
    }
}
