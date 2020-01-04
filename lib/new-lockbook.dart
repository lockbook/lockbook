import 'package:flutter/cupertino.dart';

class NewLockbook extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: NewLockbookHome(),
    );
  }
}

class NewLockbookHome extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return CupertinoPageScaffold(
      navigationBar: CupertinoNavigationBar(
        middle: const Text('New Lockbook'),
      ),
      child: Container(),
    );
  }
}