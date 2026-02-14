package com.github.computergraphicstools.metalanalyzer

import com.intellij.openapi.fileTypes.LanguageFileType
import javax.swing.Icon

object MetalFileType : LanguageFileType(MetalLanguage) {
    override fun getName(): String = "Metal"
    override fun getDescription(): String = "Metal Shading Language"
    override fun getDefaultExtension(): String = "metal"
    override fun getIcon(): Icon = MetalIcons.FILE
}
