package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * How the context window size of a model was obtained. A fallback stays visible:
 * the interface must not present a guess as a fact.
 */
@Serializable
enum class ContextSource {
    /** Nothing better was known; the number is a default guess. */
    @SerialName("default_guess") DEFAULT_GUESS,
    /** The provider answered with a number from its own model list. */
    @SerialName("provider_reported") PROVIDER_REPORTED,
    /** The local server answered with how much it allocated to the loaded model. */
    @SerialName("detected_allocated") DETECTED_ALLOCATED,
    /** The local server answered with the model's architectural maximum. */
    @SerialName("detected_maximum") DETECTED_MAXIMUM,
}

/** A model available in the switcher. */
@Serializable
data class ModelInfo(
    val id: String,
    val providerId: String = "",
    val displayName: String,
    val contextMax: Int,
    val maxOutput: Int,
    val pricing: Pricing,
    /** Whether it takes `effort`. Older models do not. */
    val supportsEffort: Boolean,
    /** Whether it reasons adaptively. Kept apart from effort: the provider reports them separately. */
    val supportsThinking: Boolean = false,
    val contextSource: ContextSource = ContextSource.DEFAULT_GUESS,
    /** Silence is a yes here: a model wrongly taken to see is refused once, while one wrongly taken to be blind never sees a screenshot. */
    val supportsImages: Boolean = true,
)
