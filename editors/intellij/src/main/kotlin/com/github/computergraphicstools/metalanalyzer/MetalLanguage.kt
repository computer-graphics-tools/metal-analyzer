package com.github.computergraphicstools.metalanalyzer

import com.intellij.lang.Language

object MetalLanguage : Language("Metal") {
    private fun readResolve(): Any = MetalLanguage
}
