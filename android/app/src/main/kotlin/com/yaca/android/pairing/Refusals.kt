package com.yaca.android.pairing

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import com.yaca.core.protocol.PROTOCOL_VERSION
import com.yaca.core.protocol.Refusal
import com.yaca.core.protocol.RefusalHandshakeExpected
import com.yaca.core.protocol.RefusalPairingFailed
import com.yaca.core.protocol.RefusalProtocolMismatch
import com.yaca.core.protocol.RefusalTooManyAttempts
import com.yaca.core.protocol.RefusalUnauthorized

/** One sentence per reason the computer can give, in the phone's language. */
@Composable
fun refusalMessage(reason: Refusal): String = when (reason) {
    RefusalUnauthorized -> stringResource(R.string.refusal_unauthorized)
    RefusalPairingFailed -> stringResource(R.string.refusal_pairing_failed)
    is RefusalProtocolMismatch -> stringResource(R.string.refusal_protocol_mismatch, reason.server, PROTOCOL_VERSION)
    RefusalTooManyAttempts -> stringResource(R.string.refusal_too_many_attempts)
    RefusalHandshakeExpected -> stringResource(R.string.refusal_handshake_expected)
}

@Composable
fun pairingFailureMessage(failure: PairingFailure): String = when (failure) {
    PairingFailure.InvalidLink -> stringResource(R.string.pairing_invalid_link)
    PairingFailure.Network -> stringResource(R.string.pairing_network)
    PairingFailure.Storage -> stringResource(R.string.pairing_storage)
    is PairingFailure.Refused -> refusalMessage(failure.reason)
}
