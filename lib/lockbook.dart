import 'package:client/file_description.dart';
import 'package:client/main.dart';
import 'package:client/user_info.dart';
import 'package:flutter/material.dart';

import 'editor.dart';

class Lockbook extends StatelessWidget {
  final UserInfo _userInfo;

  const Lockbook(this._userInfo);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: theme(),
      home: LockbookHome(_userInfo),
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
  List<FileDescription> _files = [];

  _LockbookState(this._userInfo);

  _updateFiles() => fileIndexRepository
      .getFilesAtPath('home')
      .then((lookup) => lookup.ifSuccessDo((list) {
            setState(() {
              _files = list;
            });
          }));

  @override
  void initState() {
    super.initState();

    _updateFiles();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(_userInfo.username),
        centerTitle: true,
      ),
      floatingActionButton: FloatingActionButton(
          backgroundColor: Monokai.Yellow,
          child: Icon(Icons.create),
          foregroundColor: Monokai.Dark,
          onPressed: () => Navigator.push(
                  context,
                  MaterialPageRoute(
                      builder: (context) => EditorPage(null))) // TODO
              .then((_) => _updateFiles())),
      body: ListView.builder(
        itemCount: _files.length,
        itemBuilder: (BuildContext context, int index) {
          final item = _files[index];
          return ListTile(
            title: Text(item.name),
            onTap: () => Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (context) =>
                            EditorPage(item)))
                .then((_) => _updateFiles()),
          );
        },
      ),
    );
  }
}
