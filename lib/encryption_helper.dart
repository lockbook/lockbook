// https://github.com/bcgit/pc-dart/blob/master/tutorials/rsa.md

import 'dart:math';
import 'dart:typed_data';

import "package:pointycastle/export.dart";

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
}
