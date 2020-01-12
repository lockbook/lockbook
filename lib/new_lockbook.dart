import 'package:client/account_helper.dart';
import 'package:client/errors.dart';
import 'package:client/lockbook.dart';
import 'package:client/user_info.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

class NewLockbook extends StatelessWidget {
  final AccountHelper accountHelper;

  const NewLockbook(this.accountHelper);

  @override
  Widget build(BuildContext context) {
    return CupertinoApp(
      home: NewLockbookHome(accountHelper),
      theme: CupertinoThemeData(brightness: Brightness.dark),
    );
  }
}

class NewLockbookHome extends StatefulWidget {
  final AccountHelper accountHelper;

  const NewLockbookHome(this.accountHelper);

  @override
  State<StatefulWidget> createState() => _NewLockbookState(accountHelper);
}

enum ButtonStatus { un_clicked, working }

class _NewLockbookState extends State<NewLockbookHome> {
  final AccountHelper accountHelper;

  _NewLockbookState(this.accountHelper);

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
                  disabledColor: Color(0xFF2D2A2F),
                  child: getButtonText(),
                  color: Color(0xff007AFF),
                  onPressed: (isEnabled())
                      ? () {
                          setState(() {
                            _buttonStatus = ButtonStatus.working;
                          });

                          // You are experiencing lag here because you are not using an isolate
                          accountHelper.newAccount(_username).then((result) {
                            result.ifSuccess(_nextScreen).ifFailure((error) {
                              _showError(error);
                              setState(() {
                                _buttonStatus = ButtonStatus.un_clicked;
                              });
                            });
                          });
                        }
                      : null,
                ),
              ),
            )
          ],
        ),
      ),
    );
  }

  _nextScreen(UserInfo userInfo) {
    Navigator.pushAndRemoveUntil(
        context,
        CupertinoPageRoute(builder: (context) => Lockbook(userInfo)),
        (Route<dynamic> route) => false);
  }

  _showError(UIError error) {
    showCupertinoDialog(
        context: context,
        builder: (BuildContext context) {
          return CupertinoAlertDialog(
            title: Text(error.title),
            content: Text(error.description),
            actions: [
              CupertinoDialogAction(
                  isDestructiveAction: true,
                  onPressed: () => Navigator.pop(context, 'Allow'),
                  isDefaultAction: true,
                  child: new Text("Close"))
            ],
          );
        });
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
