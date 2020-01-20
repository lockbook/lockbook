import 'dart:convert';

import 'package:client/file_description.dart';
import 'package:flutter/material.dart';
import 'package:zefyr/zefyr.dart';

import 'main.dart';

class EditorPage extends StatefulWidget {
  final FileDescription _fileDescription;

  EditorPage(this._fileDescription);

  @override
  EditorPageState createState() => EditorPageState(_fileDescription);
}

class EditorPageState extends State<EditorPage> {
  ZefyrController _controller;
  FocusNode _focusNode;

  FileDescription _fileDescription;
  String _name;

  EditorPageState(this._fileDescription);

  @override
  void initState() {
    super.initState();
    // Here we must load the document and pass it to Zefyr controller.
    if (_fileDescription != null) {
      _loadDocument(_fileDescription);
      setState(() {
        _name = _fileDescription.name;
      });
      _loadDocument(_fileDescription).then((document) {
        setState(() {
          _controller = ZefyrController(document);
        });
      });
    } else {
      _controller = ZefyrController(NotusDocument());
    }
    _focusNode = FocusNode();
  }

  @override
  Widget build(BuildContext context) {
    final Widget body = (_controller == null)
        ? Center(child: CircularProgressIndicator())
        : ZefyrScaffold(
            child: ZefyrTheme(
              child: ZefyrEditor(
                padding: EdgeInsets.all(16),
                controller: _controller,
                focusNode: _focusNode,
              ),
              data: zefyrTheme(),
            ),
          );

    return Scaffold(
      appBar: AppBar(
        title: TextField(
          decoration: InputDecoration(hintText: 'Name Document'),
          onChanged: (text) {
            setState(() {
              _name = text;
            });
          },
        ),
        actions: <Widget>[
          IconButton(
            icon: Icon(Icons.save),
            onPressed: () {
              String content = jsonEncode(_controller.document);
              fileService
                  .saveFile(
                      _fileDescription == null ? 'home' : _fileDescription.path,
                      _name == null ? 'untitled' : _name,
                      content)
                  .then((task) => task.ifFailedDo((error) =>
                      print('${error.title}, ${error.description}')));
            },
          )
        ],
      ),
      body: body,
    );
  }

  Future<NotusDocument> _loadDocument(FileDescription _fileDescription) async {
    final getContent = await fileHelper.readFromFile(_fileDescription.id);

    final createDocument = getContent
        .map((content) => NotusDocument.fromJson(jsonDecode(content)));

    createDocument.ifFailedDo((error) {
      print('${error.title}${error.description}');
    });

    return createDocument.getValueUnsafely();
  }
}
