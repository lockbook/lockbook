import 'package:client/account_helper.dart';
import 'package:client/encryption_helper.dart';
import 'package:flutter/cupertino.dart';

import 'new_lockbook.dart';

// Compile Time Constants for Dependency Injection
const String apiBase = "http://lockbook.app:8000";
const EncryptionHelper encryptionHelper = EncryptionHelper();
const AccountHelper accountHelper = AccountHelper(encryptionHelper);

const NewLockbook newLockbook = NewLockbook(accountHelper);

void main() {
  return runApp(newLockbook);
}
