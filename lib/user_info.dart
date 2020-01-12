import 'dart:convert';

import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:crypto/crypto.dart' as hash;
import 'package:pointycastle/api.dart';
import 'package:pointycastle/asymmetric/api.dart';

class RSAKeyPair {
  final BigInt modulus, publicExponent, privateExponent, p, q;

  RSAKeyPair(
      this.modulus, this.publicExponent, this.privateExponent, this.p, this.q);

  RSAKeyPair.fromAsymmetricKey(
      AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey> keyPair)
      : modulus = keyPair.publicKey.modulus,
        publicExponent = keyPair.publicKey.exponent,
        privateExponent = keyPair.privateKey.exponent,
        p = keyPair.privateKey.p,
        q = keyPair.privateKey.q;

  RSAPrivateKey getPrivateKey() =>
      RSAPrivateKey(modulus, privateExponent, p, q);

  RSAPublicKey getPublicKey() => RSAPublicKey(modulus, publicExponent);

  // Because these are dynamic, you have no compile time gauruntee that this
  // Operation will succeed...
  Map<String, dynamic> toJson() => {
        'modulus': modulus.toString(),
        'publicExponent': publicExponent.toString(),
        'privateExponent': privateExponent.toString(),
        'p': p.toString(),
        'q': q.toString()
      };

  RSAKeyPair.fromJson(Map<String, dynamic> json)
      : modulus = BigInt.parse(json['modulus']),
        publicExponent = BigInt.parse(json['publicExponent']),
        privateExponent = BigInt.parse(json['privateExponent']),
        p = BigInt.parse(json['p']),
        q = BigInt.parse(json['q']);

  bool _isValid() {
    return modulus != null &&
        publicExponent != null &&
        privateExponent != null &&
        p != null &&
        q != null;
  }
}

class UserInfo {
  final String username;
  final RSAKeyPair keyPair;

  UserInfo(this.username, this.keyPair);

  hash.Digest hashedUsername() {
    return hash.sha1.convert(utf8.encode(username));
  }

  UserInfo.fromJson(Map<String, dynamic> json)
      : username = json['username'],
        keyPair = RSAKeyPair.fromJson(json['keyPair']);

  Map<String, dynamic> toJson() =>
      {'username': username, 'keyPair': keyPair.toJson()};

  bool _isValid() {
    return username != null && keyPair != null && keyPair._isValid();
  }

  String encode() {
    return jsonEncode(toJson());
  }

  static Task<UIError, UserInfo> decode(String raw) {
    try {
      final unchecked = UserInfo.fromJson(jsonDecode(raw));
      if (unchecked._isValid()) {
        return Success(unchecked);
      } else {
        return Fail(UIError("User info corrupted",
            "Could not read user info, please open an issue for help. github.com/lockbook/client"));
      }
    } catch (error) {
      return Fail(UIError("User info corrupted",
          "Could not read user info, please open an issue for help. github.com/lockbook/client"));
    }
  }
}
