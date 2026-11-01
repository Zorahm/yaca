package com.yaca.android.pairing

import com.yaca.core.protocol.Refusal

sealed interface PairingUiState {
    /** The camera reads codes. [failure] is the outcome of the last attempt, shown above the camera. */
    data class Scan(val failure: PairingFailure? = null) : PairingUiState

    /** A link came from another app: nothing is paired until the human confirms. [link] is the raw text. */
    data class Confirm(val computerName: String, val link: String) : PairingUiState

    data class Pairing(val computerName: String) : PairingUiState

    data class Paired(val fingerprint: String) : PairingUiState
}

sealed interface PairingFailure {
    /** The text is not a Yaca pairing link (a pasted sentence, a stray QR code). */
    data object InvalidLink : PairingFailure

    /** No answer from the computer: the network, or the address is wrong. */
    data object Network : PairingFailure

    /** The computer answered and refused; [reason] says why. */
    data class Refused(val reason: Refusal) : PairingFailure

    /** Paired, but the phone could not save the credentials: the pairing is lost and must be redone. */
    data object Storage : PairingFailure
}

/** What the phone's camera can do for the scan, as the route found it. */
enum class CameraAccess { Unknown, Granted, Denied, Unavailable }
