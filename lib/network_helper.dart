import 'dart:convert';
import 'dart:typed_data';

import 'package:client/either.dart';
import 'package:client/errors.dart';
import 'package:client/file_description.dart';
import 'package:client/main.dart';
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

  Future<Either<UIError, Empty>> newAccount() async {
    final getInfo = await userRepo.getUserInfo();

    final prepBody =
        await getInfo.map(_userInfoRequestBody).flatMapFut(_userInfoRequest);

    return prepBody.map((_) => Done);
  }

  Future<Either<UIError, Empty>> uploadFile(UserInfo userInfo,
      FileDescription fileDescription, String encryptedContent) async {
    final hashed_username = userInfo.hashedUsername().toString();

    final body = {
      'id': fileDescription.id,
      'hashed_username': hashed_username,
      'auth': _generateAuthToken(hashed_username, userInfo.getPrivateKey()),
      'name': encryptionHelper.encrypt(userInfo, fileDescription.name),
      'path': encryptionHelper.encrypt(userInfo, fileDescription.path),
      'content': encryptedContent,
    };

    final response = await http.post(apiBase + "/create-file", body: body);
    print("create-file: ${response.statusCode}");

    switch (response.statusCode) {
      case 200:
        {
          return Success(Done);
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

  Future<Either<UIError, List<String>>> getFilesChangedSince(
      BigInt timestamp) async {
    List<String> ids = ['fa09ac70-3c11-11ea-ce65-05c55fdd80e5', 'f0571190-3c11-11ea-cce5-291859cb37f6'];
    return Success(ids);
  }

  Future<Either<UIError, String>> getFile(String id) async {
    final response = await http.get(apiBase + '/get-file/$id');
    return Success(response.body);
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

    return body;
  }

  Future<Either<UIError, Empty>> _userInfoRequest(
      Map<String, String> body) async {
    final response = await http.post(apiBase + "/new-account", body: body);
    print(response.statusCode);
    switch (response.statusCode) {
      case 200:
        {
          return Success(Done);
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
