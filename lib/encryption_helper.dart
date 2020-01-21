// https://github.com/bcgit/pc-dart/blob/master/tutorials/rsa.md

import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:client/errors.dart';
import 'package:client/user_info.dart';
import "package:pointycastle/export.dart";

import 'either.dart';

class EncryptionHelper {
  const EncryptionHelper();

  // TODO wrap Task?
  AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey> generateKeyPair() {
    var bitLength = 2048;

    // Create an RSA key generator and initialize it
    final keyGen = RSAKeyGenerator()
      ..init(ParametersWithRandom(
          RSAKeyGeneratorParameters(BigInt.parse('65537'), bitLength, 64),
          getSecureRandom()));

    // Use the generator
    final pair = keyGen.generateKeyPair();

    // Cast the generated key pair into the RSA key types
    final myPublic = pair.publicKey as RSAPublicKey;
    final myPrivate = pair.privateKey as RSAPrivateKey;

    return AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey>(myPublic, myPrivate);
  }

  SecureRandom getSecureRandom() {
    final secureRandom = FortunaRandom();

    final seedSource = Random.secure();
    final seeds = <int>[];
    for (int i = 0; i < 32; i++) {
      seeds.add(seedSource.nextInt(255));
    }
    secureRandom.seed(KeyParameter(Uint8List.fromList(seeds)));

    return secureRandom;
  }

  String encrypt(UserInfo userInfo, String content) {
    final publicKey = userInfo.getPublicKey();
    final plainTextBytes = Uint8List.fromList(content.codeUnits); // UTF -> bytes

    final encryptedBytes = _rsaEncrypt(publicKey, plainTextBytes); // bytes -> encrypted bytes
    return base64.encode(encryptedBytes); // encrypted bytes -> base64
  }

  Either<UIError, String> decrypt(UserInfo userInfo, String encrypted) {
    final privateKey = userInfo.getPrivateKey();

    final encryptedBytes = Base64Decoder().convert(encrypted); // base64 -> encrypted bytes
    final decryptedBytes = _rsaDecrypt(privateKey, encryptedBytes); // encrypted bytes -> decrypted bytes
    return Success(String.fromCharCodes(decryptedBytes)); // decrypted bytes -> UTF
  }

  Uint8List _rsaEncrypt(RSAPublicKey myPublic, Uint8List dataToEncrypt) {
    final encryptor = OAEPEncoding(RSAEngine())
      ..init(true, PublicKeyParameter<RSAPublicKey>(myPublic)); // true=encrypt

    return _processInBlocks(encryptor, dataToEncrypt);
  }

  Uint8List _rsaDecrypt(RSAPrivateKey myPrivate, Uint8List cipherText) {
    final decryptor = OAEPEncoding(RSAEngine())
      ..init(false,
          PrivateKeyParameter<RSAPrivateKey>(myPrivate)); // false=decrypt

    return _processInBlocks(decryptor, cipherText);
  }

  Uint8List _processInBlocks(AsymmetricBlockCipher engine, Uint8List input) {
    final numBlocks = input.length ~/ engine.inputBlockSize +
        ((input.length % engine.inputBlockSize != 0) ? 1 : 0);

    final output = Uint8List(numBlocks * engine.outputBlockSize);

    var inputOffset = 0;
    var outputOffset = 0;
    while (inputOffset < input.length) {
      final chunkSize = (inputOffset + engine.inputBlockSize <= input.length)
          ? engine.inputBlockSize
          : input.length - inputOffset;

      outputOffset += engine.processBlock(
          input, inputOffset, chunkSize, output, outputOffset);

      inputOffset += chunkSize;
    }

    return (output.length == outputOffset)
        ? output
        : output.sublist(0, outputOffset);
  }
}
