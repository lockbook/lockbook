package app.lockbook

import java.util.*

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

fun generateId(): String = UUID.randomUUID().toString()

const val path = "/tmp/lockbook/"