import 'dart:convert';
import 'dart:typed_data';

import 'package:client/either.dart';
import 'package:client/errors.dart';
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

  Future<Either<UIError, void>> newAccount() async {
    final getInfo = await userRepo.getUserInfo();

    final prepBody =
        await getInfo.map(_userInfoRequestBody).flatMapFut(_userInfoRequest);

    return prepBody;
  }

  Map<String, String> _userInfoRequestBody(UserInfo info) {
    final hashedUsername = info.hashedUsername().toString();
    final rsaPubN = info.getPublicKey().n.toString();
    final rsaPubE = info.getPublicKey().e.toString();

    final body = {
      'hashed_username': hashedUsername,
      'pub_key_n': rsaPubN,
      'pub_key_e': rsaPubE,
      'auth': _generateAuthToken(hashedUsername, info.getPrivateKey())
    };

    print(body);

    return body;
  }

  Future<Either<UIError, void>> _userInfoRequest(
      Map<String, String> body) async {
    final response = await http.post(apiBase + "/new-account", body: body);
    switch (response.statusCode) {
      case 202:
        {
          return Success(null);
        }
      case 409:
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
    final sig = signer.generateSignature(
        Uint8List.fromList(utf8.encode(hashedUsername + "," + _timestamp())));
    final bytes = sig.bytes;
    return Base64Encoder().convert(bytes);
  }

  String _timestamp() {
    return new DateTime.now().millisecondsSinceEpoch.toString();
  }
}
