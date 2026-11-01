@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.pairing

import android.Manifest
import android.content.ClipboardManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.yaca.android.R
import com.yaca.android.appViewModelFactory
import com.yaca.android.ui.theme.YacaTheme
import com.yaca.core.protocol.RefusalPairingFailed

/** The route: owns the camera permission and the clipboard, which the view model must not touch. */
@Composable
fun PairingRoute(onPaired: (String) -> Unit, onBack: () -> Unit) {
    val vm: PairingViewModel = viewModel(factory = appViewModelFactory { PairingViewModel(it, Build.MODEL) })
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var camera by rememberSaveable { mutableStateOf(cameraAccessNow(context)) }
    val requestCamera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        camera = if (granted) CameraAccess.Granted else CameraAccess.Denied
    }

    LaunchedEffect(state) {
        val paired = state as? PairingUiState.Paired ?: return@LaunchedEffect
        onPaired(paired.fingerprint)
    }

    PairingScreen(
        state = state,
        camera = camera,
        onAllowCamera = { requestCamera.launch(Manifest.permission.CAMERA) },
        onCameraUnavailable = { camera = CameraAccess.Unavailable },
        onCode = vm::onScanned,
        onPasteLink = { vm.onPasted(clipboardText(context)) },
        onConfirmIncoming = vm::confirmIncoming,
        onCancelIncoming = vm::cancelIncoming,
        onBack = onBack,
    )
}

/** Stateless: the content of [state] over a top bar with back. */
@Composable
fun PairingScreen(
    state: PairingUiState,
    camera: CameraAccess,
    onAllowCamera: () -> Unit,
    onCameraUnavailable: () -> Unit,
    onCode: (String) -> Unit,
    onPasteLink: () -> Unit,
    onConfirmIncoming: () -> Unit,
    onCancelIncoming: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.pairing_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(horizontal = 16.dp, vertical = 8.dp)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
        ) {
            when (state) {
                is PairingUiState.Scan -> ScanContent(
                    scan = state,
                    camera = camera,
                    onCode = onCode,
                    onAllowCamera = onAllowCamera,
                    onUnavailable = onCameraUnavailable,
                    onPasteLink = onPasteLink,
                )
                is PairingUiState.Confirm -> ConfirmContent(
                    confirm = state,
                    onConfirm = onConfirmIncoming,
                    onCancel = onCancelIncoming,
                )
                is PairingUiState.Pairing -> PairingProgress(state.computerName)
                is PairingUiState.Paired -> PairingProgress(computerName = null)
            }
        }
    }
}

private val sampleLink = "yaca://pair?name=ZORAH-PC"

@Preview(showBackground = true)
@Composable
private fun PairingScanPreview() {
    YacaTheme {
        PairingScreen(
            state = PairingUiState.Scan(failure = PairingFailure.Refused(RefusalPairingFailed)),
            camera = CameraAccess.Denied,
            onAllowCamera = {},
            onCameraUnavailable = {},
            onCode = {},
            onPasteLink = {},
            onConfirmIncoming = {},
            onCancelIncoming = {},
            onBack = {},
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun PairingConfirmPreview() {
    YacaTheme {
        PairingScreen(
            state = PairingUiState.Confirm(computerName = "ZORAH-PC", link = sampleLink),
            camera = CameraAccess.Unknown,
            onAllowCamera = {},
            onCameraUnavailable = {},
            onCode = {},
            onPasteLink = {},
            onConfirmIncoming = {},
            onCancelIncoming = {},
            onBack = {},
        )
    }
}

private fun cameraAccessNow(context: Context): CameraAccess =
    if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
        CameraAccess.Granted
    } else {
        CameraAccess.Unknown
    }

/** The clipboard's first text, read only when the human taps paste. */
private fun clipboardText(context: Context): String? {
    val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return null
    val clip = clipboard.primaryClip ?: return null
    if (clip.itemCount == 0) return null
    return clip.getItemAt(0).coerceToText(context)?.toString()
}
