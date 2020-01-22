import 'dart:convert';

import 'package:client/errors.dart';
import 'package:crypto/crypto.dart' as hash;
import 'package:pointycastle/api.dart';
import 'package:pointycastle/asymmetric/api.dart';

import 'either.dart';

class UserInfo {
  final String username;
  final BigInt modulus, publicExponent, privateExponent, p, q;

  UserInfo(this.username, this.modulus, this.publicExponent,
      this.privateExponent, this.p, this.q);

  UserInfo.fromAsymmetricKey(
      this.username, AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey> keyPair)
      : modulus = keyPair.publicKey.modulus,
        publicExponent = keyPair.publicKey.exponent,
        privateExponent = keyPair.privateKey.exponent,
        p = keyPair.privateKey.p,
        q = keyPair.privateKey.q;

  RSAPrivateKey getPrivateKey() =>
      RSAPrivateKey(modulus, privateExponent, p, q);

  RSAPublicKey getPublicKey() => RSAPublicKey(modulus, publicExponent);

  Map<String, String> toMap() => {
        'username': username.toString(),
        'modulus': modulus.toString(),
        'public_exponent': publicExponent.toString(),
        'private_exponent': privateExponent.toString(),
        'p': p.toString(),
        'q': q.toString()
      };

  static Either<UIError, UserInfo> fromMap(Map map) {
    try {
      final user = UserInfo(
        map['username'] as String,
        BigInt.parse(map['modulus'] as String),
        BigInt.parse(map['public_exponent'] as String),
        BigInt.parse(map['private_exponent'] as String),
        BigInt.parse(map['p'] as String),
        BigInt.parse(map['q'] as String),
      );

      if (user._isValid()) {
        return Success(user);
      } else {
        return Fail(userDecodingError('map missing values: $map'));
      }
    } catch (e) {
      return Fail(userDecodingError(e));
    }
  }

  hash.Digest hashedUsername() {
    return hash.sha1.convert(utf8.encode(username));
  }

  bool _isValid() {
    return username != null &&
        modulus != null &&
        publicExponent != null &&
        privateExponent != null &&
        p != null &&
        q != null;
  }
}
