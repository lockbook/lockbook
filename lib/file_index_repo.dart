import 'package:client/either.dart';
import 'package:client/file_description.dart';
import 'package:sqflite/sqlite_api.dart';
import 'package:uuid/uuid.dart';

import 'db_provider.dart';
import 'errors.dart';

class FileIndexRepository {
  final DBProvider _dbProvider;

  const FileIndexRepository(this._dbProvider);

  // TODO some of these could be Option's instead of eithers
  Future<Either<UIError, FileDescription>> getOrCreateFileDescriptor(
      String path, String name) async {
    final maybeExists = await _getFileDescriptor(path, name);

    if (maybeExists.isSuccessful()) {
      return maybeExists;
    } else {
      return _createFileDescriptor(path, name);
    }
  }

  Future<Either<UIError, List<FileDescription>>> getFilesAtPath(
      String path) async {
    final connection = await _dbProvider.connectToDB();

    final files = connection.flatMapFut<List<FileDescription>>(
        (db) => _getFilesAtPathQuery(db, path));

    return files;
  }

  Future<Either<UIError, List<FileDescription>>> _getFilesAtPathQuery(
      Database db, String path) async {
    final results =
        await db.rawQuery("select * from FileIndex where path='$path'");

    final parsedResults = results
        .map(FileDescription.fromMap)
        .where((task) => task.isSuccessful())
        .map((task) => task.getValueUnsafely())
        .toList();

    return Success(parsedResults);
  }

  Future<Either<UIError, FileDescription>> _createFileDescriptor(
      String path, String name) async {
    final connected = await _dbProvider.connectToDB();

    final insertResult = await connected.flatMapFut<FileDescription>(
        (db) => _createFileDescriptorQuery(db, path, name));

    return insertResult;
  }

  Future<Either<UIError, FileDescription>> _getFileDescriptor(
      String path, String name) async {
    final connected = await _dbProvider.connectToDB();

    final queryResult = await connected.flatMapFut<Map<String, dynamic>>(
        (db) => _getFileDescriptorQuery(db, path, name));

    final convertResult = queryResult.flatMap(FileDescription.fromMap);

    return convertResult;
  }

  Future<Either<UIError, Map<String, dynamic>>> _getFileDescriptorQuery(
      Database database, String path, String name) async {
    final list = await database.rawQuery(
        "select * from FileIndex where path = '$path' and name = '$name'");

    if (list.length == 1) {
      return Success(list[0]);
    } else {
      return Fail(fileNotFound(path, name));
    }
  }

  Future<Either<UIError, FileDescription>> _createFileDescriptorQuery(
      Database database, String path, String name) async {
    final uuid = Uuid().v1();
    final file = FileDescription(uuid, name, path, BigInt.from(0));

    int insert = await database.rawInsert('''
      insert into 
        FileIndex(id, name, path, version)
        VALUES('$uuid', '$name', '$path', '0')
    ''');

    print(insert);

    if (insert > 0) {
      return Success(file);
    } else {
      return Fail(failedToInsertFile(uuid, name, path));
    }
  }
}
