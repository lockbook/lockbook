import 'package:client/either.dart';
import 'package:client/errors.dart';
import 'package:sqflite/sqflite.dart';

import 'option.dart';

class DBProvider {
  const DBProvider();

  static Option<Database> _db = None();

  Future<Either<UIError, Database>> connectToDB() async {
    try {
      _db = Some(_db.getOrElse(await openDatabase('lockbook.db', version: 1,
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
              version TEXT)
          ''');

        await db.execute('create table LastUpdated(timestamp TEXT)');
      })));
    } catch (error) {
      _db = None();
      return Fail(
          UIError("Could not connect to your local db", "Error: $error"));
    }
    return _db.toEither(
        UIError("Could not connect to your local db", "Unknown error occured"));
  }
}
