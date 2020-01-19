import 'package:client/file_description.dart';
import 'package:client/task.dart';
import 'package:sqflite/sqlite_api.dart';
import 'package:uuid/uuid.dart';

import 'db_provider.dart';
import 'errors.dart';

class FileIndexRepository {
  final DBProvider _dbProvider;

  const FileIndexRepository(this._dbProvider);

  Future<Task<UIError, FileDescription>> getOrCreateFileDescriptor(
      String path, String name) async {
    final maybeExists = await _getFileDescriptor(path, name);

    if (maybeExists.isSuccessful()) {
      return maybeExists;
    } else {
      return _createFileDescriptor(path, name);
    }
  }

  Future<Task<UIError, FileDescription>> _createFileDescriptor(
      String path, String name) async {
    final connected = await _dbProvider.connectToDB();

    final insertResult = await connected
        .thenDoFuture((db) => _createFileDescriptorQuery(db, path, name));

    return insertResult;
  }

  Future<Task<UIError, FileDescription>> _getFileDescriptor(
      String path, String name) async {
    final connected = await _dbProvider.connectToDB();

    final queryResult = await connected
        .thenDoFuture((db) => _getFileDescriptorQuery(db, path, name));

    final convertResult = queryResult.thenDo(FileDescription.fromMap);

    return convertResult;
  }

  Future<Task<UIError, Map>> _getFileDescriptorQuery(
      Database database, String path, String name) async {
    final list = await database.rawQuery(
        "select * from FileIndex where path = '$path' and name = '$name'");

    if (list.length == 1) {
      return Success(list[0]);
    } else {
      return Fail(UIError("File not found", "No file matches $path, $name"));
    }
  }

  Future<Task<UIError, FileDescription>> _createFileDescriptorQuery(
      Database database, String path, String name) async {
    final uuid = Uuid().v1();
    final file = FileDescription(uuid, name, path, 0);

    int insert = await database.rawInsert('''
      insert into 
        FileIndex(id, name, path, version)
        VALUES('$uuid', '$name', '$path', 0)
    ''');

    if (insert == 1) {
      return Success(FileDescription(uuid, name, path, 0));
    } else {
      return Fail(UIError('Failed to insert',
          'Failed to insert $uuid, $name, $path, 0 into FileIndex'));
    }
  }
}
