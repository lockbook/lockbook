import 'package:client/new_lockbook.dart';
import 'package:flutter/material.dart';

import 'main.dart';

class Welcome extends StatelessWidget {
  const Welcome();

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: WelcomeHome(),
    );
  }
}

class WelcomeHome extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => _WelcomeState();
}

class _WelcomeState extends State<WelcomeHome> {
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: theme(),
      home: Scaffold(
        body: Padding(
          padding: EdgeInsets.all(15.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              const Text(
                "Lockbook.",
                textAlign: TextAlign.center,
                style: TextStyle(
                  fontFamily: "courier",
                  fontFamilyFallback: ["monospace"],
                  fontSize: 24,
                ),
              ),
              Container(
                height: 100,
              ),
              RaisedButton(
                child: Text('New Lockbook'),
                textColor: const Color(0xFF000000),
                onPressed: () => Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (context) => NewLockbook(accountHelper))),
              ),
              Container(
                height: 5,
              ),
              RaisedButton(
                textColor: const Color(0xFF000000),
                child: Text('Import Lockbook'),
                onPressed: () {},
              ),
            ],
          ),
        ),
      ),
    );
  }
}
