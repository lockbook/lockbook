import 'dart:convert';

import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:client/user_info.dart';
import 'package:client/user_repository.dart';
import 'package:http/http.dart' as http;
import 'package:pointycastle/api.dart';
import 'package:pointycastle/asymmetric/api.dart';
import 'package:pointycastle/digests/sha256.dart';
import 'package:pointycastle/signers/rsa_signer.dart';

class NetworkHelper {
  final String apiBase;
  final UserRepository userRepo;

  const NetworkHelper(this.apiBase, this.userRepo);

  Future<Task<UIError, void>> newAccount() async {
    final getInfo = await userRepo.getUserInfo();

    final prepBody = await getInfo
        .convertValue(_userInfoRequestBody)
        .thenDoFuture(_userInfoRequest);

    return prepBody;
  }

  Map<String, String> _userInfoRequestBody(UserInfo info) {
    final hashedUsername = info.hashedUsername().toString();
    final rsaPubN = info.getPublicKey().n.toString();
    final rsaPubE = info.getPublicKey().e.toString();

    final body = {
      'hashed_username': hashedUsername,
      'rsa_pub_n': rsaPubN,
      'rsa_pub_e': rsaPubE,
      'auth': _generateAuthToken(hashedUsername, info.getPrivateKey())
    };

    print(body);

    return body;
  }

  Future<Task<UIError, void>> _userInfoRequest(Map<String, String> body) async {
    final response = await http.post(apiBase + "/new-lockbook", body: body);
    switch (response.statusCode) {
      case 202:
        {
          return Success(null);
        }
      case 406:
        {
          return Fail(UIError(
              "Username Unavailable", "Please select a different username"));
        }
      default:
        {
          return Fail(UIError("Server Unavailable",
              "Please check status.lockbook.app or try again"));
        }
    }
  }

  // Requires Task?
  String _generateAuthToken(String hashedUsername, RSAPrivateKey privateKey) {
    final signer = RSASigner(SHA256Digest(), '0609608648016503040201');
    signer.init(true, PrivateKeyParameter<RSAPrivateKey>(privateKey));
    final sig = signer
        .generateSignature(utf8.encode(hashedUsername + "," + _timestamp()));
    final bytes = sig.bytes;
    return Base64Encoder().convert(bytes);
  }

  String _timestamp() {
    return new DateTime.now().millisecondsSinceEpoch.toString();
  }
}
