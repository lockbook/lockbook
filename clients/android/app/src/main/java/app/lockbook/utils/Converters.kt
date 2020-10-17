package app.lockbook.utils

import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok

val initLoggerConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val exportAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.string("content")
            if (ok != null) {
                Ok(ok)
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<Account>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val setLastSyncedConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getRootConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getChildrenConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.array<FileMetadata>("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonArray<FileMetadata>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getFileByIdConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<DecryptedValue>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val calculateSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<WorkCalculated>(ok))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        "Ok" -> Ok(Unit)
        "Err" -> matchError(jv)
        else -> Err(CoreError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

private fun matchError(jv: JsonValue): Err<CoreError> {
    return when (jv.obj?.obj("content")?.string("tag")) {
        "UiError" -> {
            val error = jv.obj?.obj("content")?.string("content")
            if (error != null) {
                Err(matchErrorName(error))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        "Unexpected" -> {
            val error = jv.obj?.obj("content")?.string("content")
            if (error != null) {
                Err(CoreError.Unexpected(error))
            } else {
                Err(CoreError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        else -> Err(CoreError.Unexpected("Can't recognize tag."))
    }
}
