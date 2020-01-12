import 'package:client/main.dart';
import 'package:client/persistence_helper.dart';
import 'package:client/user_info.dart';
import 'package:flutter/cupertino.dart';

import 'lockbook.dart';

class Welcome extends StatelessWidget {
  final PersistenceHelper persistenceHelper;

  const Welcome(this.persistenceHelper);

  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: WelcomeHome(),
      theme: CupertinoThemeData(brightness: Brightness.dark),
    );
  }
}

class WelcomeHome extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => _WelcomeState();
}

class _WelcomeState extends State<WelcomeHome> with WidgetsBindingObserver {
  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
        theme: CupertinoThemeData(brightness: Brightness.dark),
        home: CupertinoPageScaffold(
          navigationBar: CupertinoNavigationBar(
            backgroundColor: Color(0xff1C1C1E),
            middle: const Text(
              'Lockbook',
            ),
          ),
          child: Container(),
        ));
  }

}
