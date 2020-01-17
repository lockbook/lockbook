import 'package:client/account_helper.dart';
import 'package:client/encryption_helper.dart';
import 'package:client/network_helper.dart';
import 'package:client/persistence_helper.dart';
import 'package:client/welcome.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

import 'lockbook.dart';
import 'new_lockbook.dart';

// Compile Time Constants for Dependency Injection
const String apiBase = "http://lockbook.app:8000";
const EncryptionHelper encryptionHelper = EncryptionHelper();
const PersistenceHelper persistenceHelper = PersistenceHelper();
const NetworkHelper networkHelper = NetworkHelper(apiBase, persistenceHelper);
const AccountHelper accountHelper =
    AccountHelper(encryptionHelper, persistenceHelper, networkHelper);

const welcome = Welcome(persistenceHelper);
const NewLockbook newLockbook = NewLockbook(accountHelper);

void main() {
  WidgetsFlutterBinding.ensureInitialized();

  persistenceHelper.getUserInfo().then((result) => result
      .ifSuccess((info) => runApp(Lockbook(info)))
      .ifFailure((_) => runApp(welcome)));
}

theme() => ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: const Color(0xFF2C292D),
      buttonColor: const Color(0xFFFFD866),
    );
