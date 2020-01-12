import 'package:client/user_info.dart';
import 'package:flutter/cupertino.dart';

class Lockbook extends StatelessWidget {
  final UserInfo _userInfo;

  const Lockbook(this._userInfo);

  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: LockbookHome(_userInfo),
      theme: CupertinoThemeData(brightness: Brightness.dark),
    );
  }
}

class LockbookHome extends StatefulWidget {
  final UserInfo _userInfo;

  const LockbookHome(this._userInfo);

  @override
  State<StatefulWidget> createState() => _LockbookState(_userInfo);
}

class _LockbookState extends State<LockbookHome> {
  final UserInfo _userInfo;

  _LockbookState(this._userInfo);

  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
        theme: CupertinoThemeData(brightness: Brightness.dark),
        home: CupertinoPageScaffold(
          navigationBar: CupertinoNavigationBar(
            backgroundColor: Color(0xff1C1C1E),
            middle: Text(_userInfo.username),
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
