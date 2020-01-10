import 'dart:convert';

import 'package:client/encryption_helper.dart';
import 'package:client/main.dart';
import 'package:crypto/crypto.dart';
import 'package:pointycastle/api.dart';
import 'package:pointycastle/asymmetric/api.dart';
import 'package:http/http.dart' as http;
import 'package:pointycastle/digests/sha256.dart';
import 'package:pointycastle/signers/rsa_signer.dart';

class AccountHelper {
  final EncryptionHelper encryptionHelper;

  const AccountHelper(this.encryptionHelper);

  Future<void> newAccount(String username) async {
    var keypair = encryptionHelper.generateKeyPair();
    saveKeypair(keypair);

    final publicKey = keypair.publicKey;

    final hashedUsername = sha1.convert(utf8.encode(username)).toString();
    final rsaPubN = publicKey.n.toString();
    final rsaPubE = publicKey.e.toString();
    final signature = _signUsername(hashedUsername, keypair.privateKey);

    final body = {
      'hashed_username': hashedUsername,
      'rsa_pub_n': rsaPubN,
      'rsa_pub_e': rsaPubE,
      'auth': signature
    };

    print(body);

    final response = await http.post(apiBase + "/new-lockbook", body: body);
    switch (response.statusCode) {
      case 202:
        {
          return;
        }
      case 406:
        {
          return Future.error("Username Unavailable");
        }
      default:
        {
          return Future.error("Server Unavailable");
        }
    }
  }

  _signUsername(String hashedUsername, RSAPrivateKey privateKey) {
    final signer = RSASigner(SHA256Digest(), '0609608648016503040201');
    signer.init(true, PrivateKeyParameter<RSAPrivateKey>(privateKey));
    final sig = signer.generateSignature(utf8.encode(hashedUsername));
    final bytes = sig.bytes;
    return Base64Encoder().convert(bytes);
  }

  Future<void> saveKeypair(
      AsymmetricKeyPair<RSAPublicKey, RSAPrivateKey> keypair) async {
    // TODO
  }
}
