@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.pairing

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.yaca.android.R

/** The camera area: the live reader when allowed, otherwise why not and how to allow it. */
@Composable
fun CameraArea(
    camera: CameraAccess,
    onCode: (String) -> Unit,
    onAllowCamera: () -> Unit,
    onUnavailable: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(1f)
            .clip(MaterialTheme.shapes.large)
            .background(MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        when (camera) {
            CameraAccess.Granted -> QrCamera(
                onCode = onCode,
                onUnavailable = onUnavailable,
                modifier = Modifier.fillMaxWidth().aspectRatio(1f),
            )
            CameraAccess.Unknown, CameraAccess.Denied -> Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val text = if (camera == CameraAccess.Denied) {
                    stringResource(R.string.pairing_camera_denied)
                } else {
                    stringResource(R.string.pairing_camera_rationale)
                }
                Text(text, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center)
                Spacer(Modifier.height(16.dp))
                Button(onClick = onAllowCamera) {
                    Text(stringResource(R.string.pairing_camera_allow))
                }
            }
            CameraAccess.Unavailable -> Text(
                text = stringResource(R.string.pairing_camera_unavailable),
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(24.dp),
            )
        }
    }
}

/** Scan state: the camera, the note on it, the failure of the last try, and the paste alternative. */
@Composable
fun ScanContent(
    scan: PairingUiState.Scan,
    camera: CameraAccess,
    onCode: (String) -> Unit,
    onAllowCamera: () -> Unit,
    onUnavailable: () -> Unit,
    onPasteLink: () -> Unit,
) {
    Text(stringResource(R.string.pairing_heading), style = MaterialTheme.typography.headlineSmall)
    Spacer(Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.pairing_body),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(16.dp))
    CameraArea(
        camera = camera,
        onCode = onCode,
        onAllowCamera = onAllowCamera,
        onUnavailable = onUnavailable,
    )
    Spacer(Modifier.height(12.dp))
    Text(
        text = stringResource(R.string.pairing_camera_note),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    scan.failure?.let { failure ->
        Spacer(Modifier.height(12.dp))
        Text(
            text = pairingFailureMessage(failure),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
        )
    }
    Spacer(Modifier.height(16.dp))
    OutlinedButton(onClick = onPasteLink, modifier = Modifier.fillMaxWidth()) {
        Icon(Icons.Filled.ContentPaste, contentDescription = null, modifier = Modifier.padding(end = 8.dp))
        Text(stringResource(R.string.pairing_paste))
    }
}

/** A link from another app: the human reads the name of the computer and confirms or cancels. */
@Composable
fun ConfirmContent(
    confirm: PairingUiState.Confirm,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    Text(
        text = stringResource(R.string.pairing_confirm_title, confirm.computerName),
        style = MaterialTheme.typography.headlineSmall,
    )
    Spacer(Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.pairing_confirm_body),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(24.dp))
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(onClick = onCancel) { Text(stringResource(R.string.action_cancel)) }
        Spacer(Modifier.width(8.dp))
        Button(onClick = onConfirm) { Text(stringResource(R.string.pairing_confirm_action)) }
    }
}

/** While the handshake runs: a spinner and the name of the computer. */
@Composable
fun PairingProgress(computerName: String?) {
    Spacer(Modifier.height(32.dp))
    Row(verticalAlignment = Alignment.CenterVertically) {
        CircularProgressIndicator(modifier = Modifier.padding(end = 16.dp))
        if (computerName != null) {
            Text(stringResource(R.string.pairing_in_progress, computerName), style = MaterialTheme.typography.bodyLarge)
        }
    }
}
