package app.lockbook

import android.content.res.Resources
import app.lockbook.core.calculateWork
import app.lockbook.core.createAccount
import app.lockbook.model.CoreModel
import app.lockbook.model.CoreModel.getAPIURL
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.*
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test
import kotlinx.serialization.json.*
import kotlinx.serialization.modules.*
import kotlinx.serialization.PolymorphicSerializer.*
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.encoding.decodeStructure
import kotlinx.serialization.encoding.encodeStructure

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
abstract class InterCoreResult<O> {
    @Serializable
    @SerialName("Ok")
    class Ok<O>(val content: O) : InterCoreResult<O>()

    @Serializable
    @SerialName("Err")
    class Err(val content: CError) : InterCoreResult<Unit>()

    inline fun <reified T : UiCoreError> toResult(): com.github.michaelbull.result.Result<O, CoreError<T>> {
        return when (this) {
            is Ok -> {
                if (content != null) {
                    com.github.michaelbull.result.Ok(content)
                } else {
                    com.github.michaelbull.result.Ok(Unit as O)
                }
            }
            is InterCoreResult.Err -> when(content) {
                is CError.UiError -> {
                    com.github.michaelbull.result.Err(CoreError.UiError(enumValueOf<Enum<T>>(content.content)))
                }
                is CError.Unexpected -> {
                    com.github.michaelbull.result.Err(CoreError.Unexpected(content.content))
                }
            }
            else -> com.github.michaelbull.result.Err(CoreError.Unexpected("ERROR"))
        }
    }
}

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
sealed class CError {
    @Serializable
    @SerialName("UiError")
    class UiError(val content: String) : CError()

    @Serializable
    @SerialName("Unexpected")
    class Unexpected(val content: String) : CError()
}

//class IntermCoreResultSerializer<T>(private val dataSerializer: KSerializer<T>) : KSerializer<InterCoreResult.Ok<T>> {
//    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("Ok") {
//        dataSerializer.descriptor
//    }
//    override fun serialize(encoder: Encoder, value: InterCoreResult.Ok<T>) = dataSerializer.serialize(encoder, value.content)
//    override fun deserialize(decoder: Decoder) = InterCoreResult.Ok(dataSerializer.deserialize(decoder))
//}

//class IntermCoreResultSerializerStrategy<T>() : SerializationStrategy<InterCoreResult<T>> {
//
//}

class CreateAccountTest {
    var config = Config(createRandomPath())

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        config = Config(createRandomPath())
    }

    @Test
    fun createAccountOk() {
        val serializationModule = SerializersModule {
            polymorphic(InterCoreResult::class) {
                subclass(InterCoreResult.Ok.serializer(Account.serializer()))
                subclass(InterCoreResult.Err.serializer(CError.serializer()))
            }
        }

        val second = SerializersModule {
            polymorphic(InterCoreResult::class) {
                subclass(InterCoreResult.Ok.serializer(WorkCalculated.serializer()))
                subclass(InterCoreResult.Err.serializer(CError.serializer()))
            }
        }

        val three = SerializersModule {
            polymorphic(InterCoreResult::class) {
                subclass(InterCoreResult.Ok.serializer(Unit.serializer()))
                subclass(InterCoreResult.Err.serializer(CError.serializer()))
            }
        }


        val a = calculateWork(Json.encodeToString(config))

        println("Here $a")

//        backgroundSync(jsonParser.encodeToString(config))
        Json{ serializersModule = serializationModule }.decodeFromString<InterCoreResult<Account, CreateAccountError>>(createAccount(
            Json.encodeToString(config),
            generateAlphaString(),
            getAPIURL()
        ))
        Json{ serializersModule = second }.decodeFromString<InterCoreResult<WorkCalculated, CalculateWorkError>>(calculateWork(Json.encodeToString(config)))
        println("WHY: ${(Json{ serializersModule = three }.decodeFromString<InterCoreResult<Unit, SyncAllError>>(a).toResult().component2()!! as CError.UiError).content}")
    }

//    class PolymorphicEnumSerializer<T : Enum<T>>( private val enumSerializer: KSerializer<T> ) : KSerializer<T>
//    {
//        override val descriptor: SerialDescriptor = buildClassSerialDescriptor( enumSerializer.descriptor.serialName )
//        {
//            element( "value", enumSerializer.descriptor )
//        }
//
//        override fun deserialize( decoder: Decoder ): T =
//            decoder.decodeStructure(descriptor) {
//                decodeElementIndex(descriptor)
//                return decodeSerializableElement( descriptor, 0, enumSerializer )
//            }
//
//        override fun serialize( encoder: Encoder, value: T ) =
//            encoder.encodeStructure(descriptor) {
//                encodeSerializableElement( descriptor, 0, enumSerializer, value )
//            }
//    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()

        CoreModel.createAccount(config, username).unwrap()

        config = Config(createRandomPath())

//        CoreModel.createAccount(config, username)
//            .unwrapErrorType(CreateAccountError.UsernameTaken)
    }

    @Test
    fun createAccountInvalidUsername() {
//        CoreModel.createAccount(config, "!@#$%^&*()")
//            .unwrapErrorType(CreateAccountError.InvalidUsername)
    }

    @Test
    fun createAccountExistsAlready() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

//        CoreModel.createAccount(config, generateAlphaString())
//            .unwrapErrorType(CreateAccountError.AccountExistsAlready)
    }

    @Test
    fun createAccountUnexpectedError() {
//        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, CreateAccountError>>(
//            createAccount("", "", getAPIURL())
//        ).unwrapUnexpected()
    }
}
