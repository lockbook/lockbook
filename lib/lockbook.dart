import 'package:client/main.dart';
import 'package:client/user_info.dart';
import 'package:flutter/material.dart';
import 'package:quill_delta/quill_delta.dart';
import 'package:zefyr/zefyr.dart';

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

  _LockbookState(this._userInfo);

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
              context, MaterialPageRoute(builder: (context) => EditorPage()))),
      body: Container(),
    );
  }
}

class EditorPage extends StatefulWidget {
  @override
  EditorPageState createState() => EditorPageState();
}

class EditorPageState extends State<EditorPage> {
  /// Allows to control the editor and the document.
  ZefyrController _controller;

  /// Zefyr editor like any other input field requires a focus node.
  FocusNode _focusNode;

  @override
  void initState() {
    super.initState();
    // Here we must load the document and pass it to Zefyr controller.
    final document = _loadDocument();
    _controller = ZefyrController(document);
    _focusNode = FocusNode();
  }

  @override
  Widget build(BuildContext context) {
    // Note that the editor requires special `ZefyrScaffold` widget to be
    // one of its parents.
    return Scaffold(
      appBar: AppBar(title: Text("Editor page")),
      body: ZefyrScaffold(
        child: ZefyrEditor(
          padding: EdgeInsets.all(16),
          controller: _controller,
          focusNode: _focusNode,
        ),
      ),
    );
  }

  /// Loads the document to be edited in Zefyr.
  NotusDocument _loadDocument() {
    // For simplicity we hardcode a simple document with one line of text
    // saying "Zefyr Quick Start".
    // (Note that delta must always end with newline.)
    final Delta delta = Delta()..insert("Zefyr Quick Start\n");
    return NotusDocument.fromDelta(delta);
  }
}
