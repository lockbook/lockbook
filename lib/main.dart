import 'package:client/account_helper.dart';
import 'package:client/encryption_helper.dart';
import 'package:client/network_helper.dart';
import 'package:client/persistence_helper.dart';
import 'package:client/welcome.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

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
  SystemChrome.setSystemUIOverlayStyle(SystemUiOverlayStyle(
    systemNavigationBarColor: Monokai.Dark, // navigation bar color
    statusBarColor: Monokai.Dark, // status bar color
  ));
  persistenceHelper.getUserInfo().then((result) => result
      .ifSuccess((info) => runApp(Lockbook(info)))
      .ifFailure((_) => runApp(welcome)));
}

theme() => ThemeData(
      brightness: Brightness.dark,
      appBarTheme: AppBarTheme(color: Monokai.Dark, elevation: 0),
      scaffoldBackgroundColor: Monokai.Dark,
      buttonColor: Monokai.Yellow,
      hintColor: Monokai.Yellow,
      inputDecorationTheme: InputDecorationTheme(
        labelStyle: TextStyle(color: Monokai.Yellow),
        focusColor: Monokai.Yellow,
        hoverColor: Monokai.Yellow,
        fillColor: Monokai.Yellow,
        enabledBorder: UnderlineInputBorder(
          borderSide: BorderSide(color: Monokai.Yellow),
        ),
        focusedBorder: UnderlineInputBorder(
          borderSide: BorderSide(color: Monokai.Yellow),
        ),
      ),
    );

class Monokai {
  static Color White = const Color(0xFFFFFFFF);
  static Color Dark = const Color(0xFF2C292D);
  static Color Yellow = const Color(0xFFFFD866);
  static Color Green = const Color(0xFFA9DC76);
  static Color Red = const Color(0xFFFF6188);
  static Color Blue = const Color(0xFF78DCE8);
  static Color Purple = const Color(0xFFAB9DF2);
}
