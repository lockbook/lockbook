import 'package:client/account_helper.dart';
import 'package:client/lockbook.dart';
import 'package:client/user_info.dart';
import 'package:flutter/material.dart';

import 'main.dart';

class NewLockbook extends StatelessWidget {
  final AccountHelper accountHelper;

  const NewLockbook(this.accountHelper);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Monokai.Dark,
      body: NewLockbookHome(accountHelper),
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
  String _errorText;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: theme(),
      home: Scaffold(
        resizeToAvoidBottomPadding: false,
        appBar: AppBar(
          centerTitle: true,
          leading: new IconButton(
            icon: new Icon(Icons.chevron_left, color: Colors.white),
            onPressed: () => Navigator.of(context).pop(),
          ),
          title: Text('New Lockbook'),
        ),
        body: Padding(
          padding: EdgeInsets.all(30.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              TextField(
                decoration: InputDecoration(
                  labelText: 'Reserve your Username',
                  errorText: _errorText
                ),
                cursorColor: Monokai.Yellow,
                onChanged: (text) {
                  setState(() {
                    _errorText = null;
                    _username = text;
                  });
                },
              ),
              Container(
                height: 20,
              ),
              Text(
                'Click the question mark to see our Human Readable document about our Trust Model.',
                style: TextStyle(
                  color: const Color(0xFF9C979C),
                ),
              ),
              Container(
                height: 20,
              ),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  IconButton(
                    color: Monokai.Blue,
                    icon: Icon(Icons.help),
                    splashColor: Monokai.Blue,
                    iconSize: 50,
                    tooltip: 'See our human readable terms of service',
                    onPressed: () {}, // TODO
                  ),
                  IconButton(
                    iconSize: 50,
                    splashColor: Monokai.Green,
                    color: Monokai.Green,
                    icon: Icon(Icons.check_box),
                    onPressed: (_isEnabled())
                        ? () {
                            setState(() {
                              _buttonStatus = ButtonStatus.working;
                            });
                            // You are experiencing lag here because you are not using an isolate
                            accountHelper.newAccount(_username).then((result) {
                              result.ifSuccessDo(_nextScreen).ifFailedDo((error) {
                                setState(() {
                                  _errorText = error.title;
                                  _buttonStatus = ButtonStatus.un_clicked;
                                });
                              });
                            });
                          }
                        : null,
                    tooltip: 'Generate your keypair and reserve your username',
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _nextScreen(UserInfo userInfo) {
    Navigator.pushAndRemoveUntil<dynamic>(
        context,
        MaterialPageRoute<dynamic>(builder: (context) => Lockbook(userInfo)),
        (Route<dynamic> route) => false);
  }

  bool _isEnabled() {
    return _buttonStatus != ButtonStatus.working && _username != "" && _errorText == null;
  }
}
