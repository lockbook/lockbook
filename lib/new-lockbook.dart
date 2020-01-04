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

class NewLockbookHome extends StatelessWidget {
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
                placeholder: 'Passphrase',
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
                  child: Text('Generate Keypair'),
                  color: Color(0xff007AFF),
                  onPressed: () {
                    print("object");
                  },
                ),
              ),
            )
          ],
        ),
      ),
    );
  }
}
