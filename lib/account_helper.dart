import 'package:client/encryption_helper.dart';

class AccountHelper {
  static Future<void> newAccount(String username) async {
    var keypair = generateKeyPair();

    var public_key = keypair.publicKey;
    var private_key = keypair.privateKey;

    
  }
}