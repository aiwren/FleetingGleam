package com.fleetinggleam.android.core

/**
 * How photos are analysed, decided here instead of asked of the person (same as the Mac's src/strategy.ts).
 *
 * - One model for the whole app: the learned taste, grouping and search all compare embeddings, which only agree when the same model made them,
 *   so the choice is made once (the first analysis) and kept.
 * - Faces: the phone has no system face detector, so the small detector (`detect`) and the MediaPipe mesh (`eyes`) come with the first download,
 *   together with people recognition (`faces`). Eyes are always judged on MediaPipe's scale here.
 * - The hardware: NNAPI when it agrees with the CPU (the engine checks), the CPU otherwise.
 */
object Strategy {
    @Volatile private var machine: Machine? = null

    suspend fun machine(): Machine = machine ?: runCatching { Core.get<Machine>("machine_info") }.getOrDefault(Machine()).also { machine = it }

    /**
     * The larger model is about 600 MB of weights and runs on the phone's CPU or NPU: worth it only on phones with the room for it. The quick model
     * otherwise; a better one can always be fetched on purpose from the models list (`adopt`).
     */
    fun capableOfHigh(m: Machine): Boolean = m.memoryGb >= 12 && m.cores >= 8

    /** The packs an analysis with this model needs. */
    fun tierPacks(t: String): List<String> = when (t) {
        "ultra" -> listOf(PackIds.STANDARD, PackIds.ULTRA)
        "high" -> listOf(PackIds.HIGH)
        else -> listOf(PackIds.STANDARD)
    }

    /**
     * 应用会定下哪一档：已经定下的；手机上已经有的高质量包；否则看手机跑不跑得动。
     * "标准包已装"不算依据（审查第 51 条）：标准包多半是示例照片顺手下的（示例不记档位），拿它当依据，
     * 高配手机以后就被定成标准模型、再也不问。
     */
    suspend fun predictTier(): String {
        Settings.value.tier?.let { return it }
        Models.refresh()
        if (Models.installed(PackIds.HIGH) && !Models.installed(PackIds.STANDARD)) return "high"
        return if (capableOfHigh(machine())) "high" else "fast"
    }

    /**
     * 记下这台手机定下的模型——只在它要的包**下载成功（或本来就在）之后**调。以前下载前就写，下载一取消档位就锁死在没装的那档（审查第 24 条）。
     */
    fun settle(tier: String) {
        if (Settings.value.tier == null) Settings.set { it.copy(tier = tier) }
    }

    /**
     * 示例照片用哪一档：已经定下的、或手机上已经有的那档（不用下载）；都没有就用最小的标准模型。不记下来——示例只是看一眼效果。
     */
    suspend fun sampleTier(): String {
        Settings.value.tier?.let { return it }
        Models.refresh()
        return if (Models.installed(PackIds.HIGH) && !Models.installed(PackIds.STANDARD)) "high" else "fast"
    }

    /** The tier to analyse with right now, without waiting: what was settled on, or the quick one while nothing is. */
    fun currentTier(): String = Settings.value.tier ?: "fast"

    /** The eye model on a phone is always MediaPipe's (the face pipeline measures with it). */
    fun eyeMethodNow(): String = "mediapipe"

    /** Everything one analysis needs: its model, and the extras (face detection, eyes, people) that make it better. Asked for under one question. */
    fun analysisPacks(t: String): List<String> = tierPacks(t) + listOf(PackIds.DETECT, PackIds.EYES, PackIds.FACES)

    /**
     * Roughly how long analysing `photos` new photos takes (seconds) on a phone: per photo, measured on an 8-core phone CPU (decoding included),
     * scaled to the cores.
     */
    fun estimateSeconds(t: String, photos: Int, cores: Int): Int {
        val perPhoto = when (t) { "high" -> 1.1; "ultra" -> 3.5; else -> 0.25 }
        return Math.round(photos * perPhoto * maxOf(1.0, 8.0 / maxOf(1, cores))).toInt()
    }

    /** The tier a pack that was just fetched on purpose (from the models list) stands for: getting a better model is asking to use it. */
    fun adopt(id: String) {
        if (id == "high" || id == "ultra") Settings.set { it.copy(tier = id, tierRev = it.tierRev + 1) }
    }

    /** A model that was in use has been removed: the app settles on one again the next time it analyses. */
    fun release(id: String) {
        if (Settings.value.tier == id) Settings.set { it.copy(tier = null, tierRev = it.tierRev + 1) }
    }
}
