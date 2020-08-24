package app.lockbook

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

const val path = "/temp/lockbook/"