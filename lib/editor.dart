import 'dart:convert';

import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:quill_delta/quill_delta.dart';
import 'package:zefyr/zefyr.dart';

import 'main.dart';

class EditorPage extends StatefulWidget {
  String _path, _file;

  EditorPage(this._path, this._file);

  @override
  EditorPageState createState() => EditorPageState(_path, _file);
}

class EditorPageState extends State<EditorPage> {
  ZefyrController _controller;
  FocusNode _focusNode;

  String _path, _name;

  EditorPageState(this._path, this._name);

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
      appBar: AppBar(
        title: TextField(
          decoration: InputDecoration(hintText: 'Name Document'),
        ),
        actions: <Widget>[
          IconButton(
            icon: Icon(Icons.save),
            onPressed: () {
              String content = jsonEncode(_controller.document);
              fileService.saveFile(_path, _name, content).then((task) =>
                  task.ifFailure((error) =>
                      print('${error.title}, ${error.description}')));
            },
          )
        ],
      ),
      body: ZefyrScaffold(
        child: ZefyrTheme(
          child: ZefyrEditor(
            padding: EdgeInsets.all(16),
            controller: _controller,
            focusNode: _focusNode,
          ),
          data: zefyrTheme(),
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
