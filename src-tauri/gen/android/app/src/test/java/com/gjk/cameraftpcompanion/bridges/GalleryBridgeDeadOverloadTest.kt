package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class GalleryBridgeDeadOverloadTest {
    @Test
    fun `throwable overload of shouldRequestDeleteConfirmation is removed`() {
        val sourceFile = resolveSourceFile(
            "src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt"
        )
        val source = sourceFile.readText()
        assertFalse(
            "shouldRequestDeleteConfirmation(apiLevel, throwable) overload should be removed",
            source.contains("throwable: Throwable)")
        )
    }

    private fun resolveSourceFile(relativePath: String): File {
        val candidates = listOf(File(relativePath), File("app/$relativePath"))
        return candidates.first { it.exists() }
    }
}
