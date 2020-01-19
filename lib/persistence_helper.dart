import 'dart:io';

import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:client/user_info.dart';
import 'package:path_provider/path_provider.dart';
import 'package:sqflite/sqflite.dart';

class PersistenceHelper {
  const PersistenceHelper();

  static Database db;

  // TODO Error handle this
  connectToDB() async {
    try {
      if (db == null) {
        db = await openDatabase('lockbook.db', version: 1,
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
              id BLOB PRIMARY KEY,
              name TEXT,
              path TEXT,
              version INTEGER)
          ''');
        });
      }
    } catch (error) {
      print('Failed to open db');
      print(error);
    }
  }

  // Perhaps an example where I'd like to programmatically differentiate between
  // "I can't access files" and "You don't have a username yet"
  Future<Task<UIError, UserInfo>> getUserInfo() async {
    await connectToDB();

    List<Map> results = await db.rawQuery('select * from UserInfo');
    if (results.length == 1) {
      return UserInfo.fromMap(results[0]);
    } else {
      return Fail(UIError("No User Info saved", "Please create a user"));
    }
  }

  Future<Task<UIError, void>> saveUserInfo(UserInfo userInfo) async {
    await connectToDB();

    int insert = await db.rawInsert('''REPLACE INTO 
        UserInfo(id, username, modulus, public_exponent, private_exponent, p, q) 
        VALUES(1, 
        "${userInfo.username.toString()}", 
        "${userInfo.modulus.toString()}", 
        "${userInfo.publicExponent.toString()}", 
        "${userInfo.privateExponent.toString()}", 
        "${userInfo.p.toString()}", 
        "${userInfo.q.toString()}")
        ''');

    print("here");

    if (insert == 1) {
      return Success(1);
    } else {
      return Fail(UIError(
          "Failed to save user info", "Failed to save private key to db"));
    }
  }

  Future<Task<UIError, Directory>> _getAppDir() async {
    final Directory directory =
        await getApplicationDocumentsDirectory().catchError((e) {
      print("Error getting application directory, prob plugin not supported");
      print(e);
      // the implementation indicates it may return null, so I'll do that too :(
      return null;
    });

    if (directory == null) {
      return Fail(UIError(
          "Unable to access file system",
          "It seems path_provider is not supported on this platform, "
              "please tell us what platform you're using, and we'll investigate: "
              "github.com/lockbook -> issues"));
    }

    return Success(directory);
  }

  Future<Task<UIError, void>> _writeToFile(
      String location, String content) async {
    final file = File(location);

    try {
      file.writeAsStringSync(content);
    } catch (error) {
      return Fail(UIError("Could not write to file",
          "Error: $error while writing to $location"));
    }
    return Success(null);
  }

  Future<Task<UIError, String>> _readFromFile(String location) async {
    final file = File(location);

    try {
      return Success(file.readAsStringSync());
    } catch (error) {
      return Fail(UIError(
          "Could not read file", "Error: $error while writing to $location"));
    }
  }
}
