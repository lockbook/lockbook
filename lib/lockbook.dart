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
  double _progress = 1;

  _LockbookState(this._userInfo);

  void _updateFiles() => fileIndexRepository
      .getFilesAtPath('home')
      .then((lookup) => lookup.ifSuccessDo((list) {
            setState(() {
              _files = list;
            });
          }));

  void _syncFiles() {}

  @override
  void initState() {
    super.initState();

    _syncFiles();
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
          onPressed: () => Navigator.push<dynamic>(
                  context,
                  MaterialPageRoute<dynamic>(
                      builder: (context) => EditorPage(null))) // TODO
              .then((dynamic _) => _updateFiles())),
      body: _progress == 1
          ? ListView.builder(
              itemCount: _files.length,
              itemBuilder: (BuildContext context, int index) {
                final item = _files[index];
                return ListTile(
                  title: Text(item.name),
                  onTap: () => Navigator.push<dynamic>(
                          context,
                          MaterialPageRoute<dynamic>(
                              builder: (context) => EditorPage(item)))
                      .then((dynamic _) => _updateFiles()),
                );
              },
            )
          : Container(),
    );
  }
}
