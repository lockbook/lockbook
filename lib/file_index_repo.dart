import 'package:client/task.dart';
import 'package:client/user_info.dart';
import 'package:sqflite/sqlite_api.dart';

import 'db_provider.dart';
import 'errors.dart';

class FileIndexRepository {
  final DBProvider _dbProvider;

  const FileIndexRepository(this._dbProvider);

  // Perhaps an example where I'd like to programmatically differentiate between
  // "I can't access files" and "You don't have a username yet"
  Future<Task<UIError, UserInfo>> getUserInfo() async {
    return (await _dbProvider.connectToDB()).thenDoFuture(_getUserInfo);
  }

  Future<Task<UIError, UserInfo>> _getUserInfo(Database db) async {
    List<Map> results = await db.rawQuery('select * from UserInfo');
    if (results.length == 1) {
      return UserInfo.fromMap(results[0]);
    } else {
      return Fail(UIError("No User Info saved", "Please create a user"));
    }
  }

  Future<Task<UIError, void>> saveFile(String name, String content) async {
    return (await _dbProvider.connectToDB())
        .thenDoFuture((db) => _saveUserInfo(db, null));
  }

  Future<Task<UIError, void>> _saveUserInfo(
      Database db, UserInfo userInfo) async {
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
}
