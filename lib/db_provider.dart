import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:sqflite/sqflite.dart';

class DBProvider {
  const DBProvider();

  static Database _db;

  Future<Either<UIError, Database>> connectToDB() async {
    try {
      if (_db == null) {
        _db = await openDatabase('lockbook.db', version: 1,
            onCreate: (Database db, int version) async {
          await db.execute('create table UserInfo ('
              'id INTEGER PRIMARY KEY,'
              'username TEXT,'
              'modulus TEXT,'
              'public_exponent TEXT,'
              'private_exponent TEXT,'
              'p TEXT,'
              'q TEXT)');

          await db.execute('''
            create table FileIndex(
              id TEXT PRIMARY KEY,
              name TEXT,
              path TEXT,
              version INTEGER)
          ''');
        });
      }
      return Success(_db);
    } catch (error) {
      return Fail(dbFailedToConnect(error));
    }
  }
}
