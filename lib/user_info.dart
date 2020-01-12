import 'dart:convert';

import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:crypto/crypto.dart' as hash;
import 'package:pointycastle/api.dart';
import 'package:pointycastle/asymmetric/api.dart';

class RSAKeyPair {
  final BigInt modulus, exponent, p, q;

  RSAKeyPair(this.modulus, this.exponent, this.p, this.q);

  RSAKeyPair.fromAsymmetricKey(
      AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey> keyPair)
      : modulus = keyPair.publicKey.modulus,
        exponent = keyPair.publicKey.exponent,
        p = keyPair.privateKey.p,
        q = keyPair.privateKey.q;

  RSAPrivateKey getPrivateKey() => RSAPrivateKey(modulus, exponent, p, q);

  RSAPublicKey getPublicKey() => RSAPublicKey(modulus, exponent);

  Map<String, dynamic> toJson() => {
        'modulus': modulus.toString(),
        'exponent': exponent.toString(),
        'p': p.toString(),
        'q': q.toString()
      };

  RSAKeyPair.fromJson(Map<String, dynamic> json)
      : modulus = BigInt.parse(json['modulus']),
        exponent = BigInt.parse(json['exponent']),
        p = BigInt.parse(json['p']),
        q = BigInt.parse(json['q']);

  bool _isValid() {
    return modulus != null && exponent != null && p != null && q != null;
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
