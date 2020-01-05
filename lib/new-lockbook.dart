import 'package:client/account_helper.dart';
import 'package:flutter/cupertino.dart';

class NewLockbook extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: NewLockbookHome(),
      theme: CupertinoThemeData(brightness: Brightness.dark),
    );
  }
}

class NewLockbookHome extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => _NewLockbookState();
}

enum ButtonStatus { un_clicked, working }

class _NewLockbookState extends State<NewLockbookHome> {
  ButtonStatus _buttonStatus = ButtonStatus.un_clicked;
  String _username = "";
  String _passphrase = "";

  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      theme: CupertinoThemeData(brightness: Brightness.dark),
      home: CupertinoPageScaffold(
        navigationBar: CupertinoNavigationBar(
          middle: const Text('Lockbook'),
          backgroundColor: Color(0xff1C1C1E),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            /// title
            Container(
              margin: const EdgeInsets.only(
                top: 32.0,
                bottom: 6.0,
              ),
              padding: const EdgeInsets.only(left: 16.0),
              child: Text(
                'Create a new Lockbook'.toUpperCase(),
                style: TextStyle(
                  fontSize: 13.0,
                ),
              ),
            ),

            /// input
            Container(
              height: 44.0,
              child: CupertinoTextField(
                onChanged: (text) {
                  setState(() {
                    _username = text;
                  });
                },
                placeholder: 'Unique Username',
                padding: const EdgeInsets.symmetric(
                  horizontal: 16.0,
                  vertical: 12.0,
                ),
                decoration: BoxDecoration(
                  color: Color(0xff1C1C1E),
                ),
                style: TextStyle(
                  fontFamily: 'Corpid',
                  fontSize: 17.0,
                ),
              ),
            ),

            Container(
              height: 1.0,
              padding: const EdgeInsets.symmetric(horizontal: 16.0),
              color: Color(0xff1C1C1E),
              child: Container(
                color: CupertinoDynamicColor.resolve(
                    CupertinoColors.separator, context),
              ),
            ),

            /// input
            Container(
              height: 44.0,
              child: CupertinoTextField(
                obscureText: true,
                onChanged: (text) {
                  setState(() {
                    _passphrase = text;
                  });
                },
                placeholder: 'Passphrase (unimplemented)',
                padding: const EdgeInsets.symmetric(
                  horizontal: 16.0,
                  vertical: 12.0,
                ),
                decoration: BoxDecoration(
                  color: Color(0xff1C1C1E),
                ),
                style: TextStyle(
                  fontFamily: 'Corpid',
                  fontSize: 17.0,
                ),
              ),
            ),

            Center(
              child: Container(
                padding: EdgeInsets.symmetric(horizontal: 10, vertical: 50),
                child: CupertinoButton(
                  onPressed: (isEnabled())
                      ? () {
                          setState(() {
                            _buttonStatus = ButtonStatus.working;
                          });

                          AccountHelper.newAccount(_username).then((_) {
                            print("success");
                          }).catchError((error) {
                            print("error");
                          }).whenComplete(() {
                            print("complete");
                            setState(() {
                              _buttonStatus = ButtonStatus.un_clicked;
                            });
                          });
                        }
                      : null,
                  disabledColor: Color(0xFF2D2A2F),
                  child: getButtonText(),
                  color: Color(0xff007AFF),
                ),
              ),
            )
          ],
        ),
      ),
    );
  }

  bool isEnabled() {
    return _username.isNotEmpty &&
        _passphrase.isNotEmpty &&
        _buttonStatus != ButtonStatus.working;
  }

  Widget getButtonText() {
    if (_username.isEmpty) {
      return Text("Enter a username");
    } else if (_passphrase.isEmpty) {
      return Text("Enter a passphrase");
    } else if (_buttonStatus == ButtonStatus.working) {
      return CupertinoActivityIndicator();
    } else {
      return Text("Create Account");
    }
  }
}
