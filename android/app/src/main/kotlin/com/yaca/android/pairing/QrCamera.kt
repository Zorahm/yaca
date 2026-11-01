package com.yaca.android.pairing

import android.content.Context
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import java.util.concurrent.ExecutionException
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * The back camera as a QR reader. Every decoded QR value goes to [onCode]; the view model decides
 * which of them is a pairing link. The camera is released when the view leaves composition.
 */
@Composable
fun QrCamera(
    onCode: (String) -> Unit,
    onUnavailable: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val owner = LocalLifecycleOwner.current
    val currentOnCode by rememberUpdatedState(onCode)
    val currentOnUnavailable by rememberUpdatedState(onUnavailable)
    val binding = remember { QrCameraBinding() }
    DisposableEffect(binding) {
        onDispose { binding.release() }
    }
    AndroidView(
        modifier = modifier,
        factory = { context ->
            PreviewView(context).also { view ->
                binding.bind(
                    context = context,
                    owner = owner,
                    view = view,
                    onCode = { currentOnCode(it) },
                    onUnavailable = { currentOnUnavailable() },
                )
            }
        },
    )
}

/** Owns the camera use cases and the analysis thread; one per composition of [QrCamera]. */
private class QrCameraBinding {
    private val analysisExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private val scanner: BarcodeScanner = BarcodeScanning.getClient(
        BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build(),
    )
    private var provider: ProcessCameraProvider? = null
    private var released = false

    fun bind(
        context: Context,
        owner: LifecycleOwner,
        view: PreviewView,
        onCode: (String) -> Unit,
        onUnavailable: () -> Unit,
    ) {
        val future = ProcessCameraProvider.getInstance(context)
        future.addListener({
            if (released) return@addListener
            try {
                val cameraProvider = future.get()
                provider = cameraProvider
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(analysisExecutor) { frame -> analyze(frame, onCode) }
                cameraProvider.unbindAll()
                cameraProvider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
            } catch (noCamera: IllegalArgumentException) {
                // bindToLifecycle throws this when the device has no back camera.
                onUnavailable()
            } catch (failed: ExecutionException) {
                onUnavailable()
            }
        }, ContextCompat.getMainExecutor(context))
    }

    @androidx.annotation.OptIn(ExperimentalGetImage::class)
    private fun analyze(frame: ImageProxy, onCode: (String) -> Unit) {
        val media = frame.image
        if (media == null) {
            frame.close()
            return
        }
        val input = InputImage.fromMediaImage(media, frame.imageInfo.rotationDegrees)
        // ML Kit delivers its results on the main thread, so the callback may touch state directly.
        scanner.process(input)
            .addOnSuccessListener { codes -> codes.forEach { code -> code.rawValue?.let(onCode) } }
            .addOnCompleteListener { frame.close() }
    }

    fun release() {
        released = true
        provider?.unbindAll()
        scanner.close()
        analysisExecutor.shutdown()
    }
}
