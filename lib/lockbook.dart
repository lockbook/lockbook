import 'package:flutter/cupertino.dart';

class Lockbook extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: LockbookHome(),
      theme: CupertinoThemeData(brightness: Brightness.dark),
    );
  }
}

class LockbookHome extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => _LockbookState();
}

class _LockbookState extends State<LockbookHome> {
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
            trailing: GestureDetector(
              onTap: () => print("create"),
              child: Icon(
                CupertinoIcons.create,
              ),
            ),
          ),
          child: Container(),
        ));
  }
}
